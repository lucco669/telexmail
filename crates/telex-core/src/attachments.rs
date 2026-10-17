//! Attachments of stored mail, opened or saved: taken out of the raw message Telex already
//! has (no download), written under a file name that is safe on Windows and Linux, and
//! marked on Windows as coming from the internet, so Office opens them in Protected View
//! and SmartScreen checks programs. Files that can run code are named, so the window can
//! ask before opening one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::mime::{attachment_at, FileAttachment};
use crate::model::MessageId;
use crate::store::{MailStore, StoreError};
use crate::tr;

/// Kinds of file that run code when opened, or open something that does: they need a second
/// click. Lowercase, without the dot.
const RISKY: &[&str] = &[
    "exe",
    "com",
    "scr",
    "pif",
    "bat",
    "cmd",
    "msi",
    "msp",
    "mst",
    "msix",
    "msixbundle",
    "appx",
    "appxbundle",
    "application",
    "appref-ms",
    "gadget",
    "cpl",
    "msc",
    "inf",
    "reg",
    "scf",
    "lnk",
    "url",
    "hta",
    "chm",
    "js",
    "jse",
    "vbs",
    "vbe",
    "wsf",
    "wsh",
    "wsc",
    "ps1",
    "psm1",
    "psd1",
    "ps1xml",
    "jar",
    "iso",
    "img",
    "vhd",
    "vhdx",
    "dll",
    "sys",
    "docm",
    "dotm",
    "xlsm",
    "xltm",
    "xlam",
    "pptm",
    "potm",
    "ppam",
    "ppsm",
    "sldm",
    "xll",
    "iqy",
    "slk",
    "one",
    "onepkg",
    "ade",
    "adp",
    "mde",
    "accde",
    "library-ms",
    "search-ms",
    "searchconnector-ms",
    "settingcontent-ms",
    "diagcab",
    "cab",
    "msu",
    "mht",
    "mhtml",
    "xbap",
    "jnlp",
    "website",
    "hlp",
    "sct",
    "vb",
    "shb",
    "sh",
    "bash",
    "run",
    "desktop",
    "appimage",
    "deb",
    "rpm",
    "py",
    "pyw",
    "pl",
    "rb",
];

/// Names Windows keeps for devices; a file can't have them, with or without an extension.
const DEVICES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Whether opening a file of this name can run code.
pub fn is_risky(name: &str) -> bool {
    let name = name
        .trim()
        .trim_end_matches(['.', ' '])
        .to_ascii_lowercase();
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| RISKY.contains(&ext))
}

/// A file name that is safe to create: no folders, no characters Windows refuses, no device
/// names, no dots or spaces at the end, at most 150 characters. An attachment named
/// "..\..\Startup\x.exe" becomes "Startup x.exe" in the folder it is saved to, never elsewhere.
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let mut name = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    name = name
        .trim_start_matches(['.', ' '])
        .trim_end_matches(['.', ' '])
        .to_owned();
    if name.chars().count() > 150 {
        // Keep the extension: it decides how the file opens.
        let ext = name
            .rsplit_once('.')
            .filter(|(_, e)| !e.is_empty() && e.len() <= 10)
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        let stem: String = name.chars().take(150 - ext.chars().count()).collect();
        name = format!("{}{ext}", stem.trim_end_matches(['.', ' ']));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.is_empty() {
        name = "attachment".to_owned();
    } else if DEVICES.contains(&stem.as_str()) {
        name = format!("_{name}");
    }
    name
}

/// Write an attachment into `dir` under its safe name, never over another file: a second
/// "faktura.pdf" becomes "faktura (2).pdf". When `reuse` is set and a file of that name
/// already holds exactly these bytes (the same attachment opened again), that one is
/// returned. On Windows the file is marked as coming from the internet.
pub fn write_file(dir: &Path, file: &FileAttachment, reuse: bool) -> io::Result<PathBuf> {
    write_named(dir, &file.name, &file.data, reuse)
}

/// Write any bytes into `dir` as [`write_file`] writes an attachment: under the safe form of
/// `name`, numbered rather than over another file, marked as coming from the internet.
pub fn write_named(dir: &Path, name: &str, data: &[u8], reuse: bool) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = safe_file_name(name);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    for n in 1..10_000 {
        let candidate = if n == 1 {
            dir.join(&name)
        } else {
            dir.join(format!("{stem} ({n}){ext}"))
        };
        if candidate.exists() {
            if reuse && fs::read(&candidate).is_ok_and(|old| old == data) {
                return Ok(candidate);
            }
            continue;
        }
        // create_new: if another program made the file just now, try the next name.
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut f) => {
                io::Write::write_all(&mut f, data)?;
                drop(f);
                mark_from_internet(&candidate);
                return Ok(candidate);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other(tr!(
        "too many files with that name",
        "za dużo plików o tej nazwie"
    )))
}

/// Write an attachment to a file the user chose in a Save dialog (which asked before
/// replacing one), marked like [`write_file`]'s.
pub fn save_as(path: &Path, file: &FileAttachment) -> io::Result<()> {
    save_bytes(path, &file.data)
}

/// Write bytes to a file the user chose in a Save dialog, marked like [`write_file`]'s.
pub fn save_bytes(path: &Path, data: &[u8]) -> io::Result<()> {
    fs::write(path, data)?;
    mark_from_internet(path);
    Ok(())
}

/// The Mark of the Web: Windows remembers that the file came from the internet, so Office
/// opens it in Protected View and SmartScreen checks it before it runs. Best effort.
#[cfg(windows)]
fn mark_from_internet(path: &Path) {
    let mut stream = path.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    let _ = fs::write(stream, "[ZoneTransfer]\r\nZoneId=3\r\n");
}

#[cfg(not(windows))]
fn mark_from_internet(_: &Path) {}

/// Where opened attachments go: a folder per message under the system's temporary folder,
/// emptied when Telex starts ([`clear_opened`]).
pub fn opened_dir(temp: &Path, message: MessageId) -> PathBuf {
    temp.join("Telex").join(message.to_string())
}

/// Remove the attachments opened earlier. Files still open in another program stay.
pub fn clear_opened(temp: &Path) {
    let _ = fs::remove_dir_all(temp.join("Telex"));
}

impl MailStore {
    /// An attachment of a stored message, by its place in the message's list.
    pub fn attachment(&self, id: MessageId, index: usize) -> Result<FileAttachment, StoreError> {
        let raw = self.raw_of(id)?.ok_or(StoreError::UnknownMessage(id))?;
        attachment_at(&raw, index).ok_or_else(|| {
            StoreError::Invalid(tr!(
                "That attachment isn't in the message.",
                "Tego załącznika nie ma w wiadomości."
            ))
        })
    }

    /// Every attachment of a stored message.
    pub fn attachments(&self, id: MessageId) -> Result<Vec<FileAttachment>, StoreError> {
        let raw = self.raw_of(id)?.ok_or(StoreError::UnknownMessage(id))?;
        Ok(crate::mime::attachments_of(&raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, data: &[u8]) -> FileAttachment {
        FileAttachment {
            name: name.into(),
            content_type: "application/octet-stream".into(),
            data: data.to_vec(),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "telex-att-{tag}-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ))
    }

    #[test]
    fn names_stay_in_their_folder() {
        assert_eq!(safe_file_name("..\\..\\Startup\\x.exe"), "Startup x.exe");
        assert_eq!(safe_file_name("../../etc/passwd"), "etc passwd");
        assert_eq!(
            safe_file_name("faktura: 10/2026?.pdf"),
            "faktura 10 2026 .pdf"
        );
        assert_eq!(safe_file_name("zdjęcie żółwia.jpg"), "zdjęcie żółwia.jpg");
        assert_eq!(safe_file_name("CON.txt"), "_CON.txt");
        assert_eq!(safe_file_name("nul"), "_nul");
        assert_eq!(safe_file_name("raport. . ."), "raport");
        assert_eq!(safe_file_name("..."), "attachment");
        assert_eq!(safe_file_name("a\u{0}b\tc.txt"), "a b c.txt");
        let long = format!("{}.pdf", "ż".repeat(400));
        let short = safe_file_name(&long);
        assert_eq!(short.chars().count(), 150);
        assert!(short.ends_with(".pdf"));
    }

    #[test]
    fn programs_are_risky_documents_are_not() {
        for risky in [
            "setup.EXE",
            "faktura.pdf.exe",
            "x.js",
            "a.lnk",
            "b.docm",
            "c.ps1",
            "d.exe. ",
            "e.one",
            "f.xll",
            "g.search-ms",
            "h.mht",
        ] {
            assert!(is_risky(risky), "{risky}");
        }
        for fine in [
            "faktura.pdf",
            "zdjęcie.jpg",
            "umowa.docx",
            "exe",
            "notatki.txt",
        ] {
            assert!(!is_risky(fine), "{fine}");
        }
    }

    #[test]
    fn files_never_overwrite_and_the_same_one_is_reused() {
        let dir = temp_dir("write");
        let a = write_file(&dir, &file("faktura.pdf", b"one"), false).unwrap();
        let b = write_file(&dir, &file("faktura.pdf", b"two"), false).unwrap();
        assert_eq!(a.file_name().unwrap(), "faktura.pdf");
        assert_eq!(b.file_name().unwrap(), "faktura (2).pdf");
        assert_eq!(fs::read(&a).unwrap(), b"one");
        // Opening the first one again finds it instead of making a third copy.
        let again = write_file(&dir, &file("faktura.pdf", b"two"), true).unwrap();
        assert_eq!(again, b);
        let third = write_file(&dir, &file("faktura.pdf", b"three"), true).unwrap();
        assert_eq!(third.file_name().unwrap(), "faktura (3).pdf");
        let odd = write_file(&dir, &file("..\\..\\boot.ini", b"x"), false).unwrap();
        assert_eq!(odd.parent().unwrap(), dir, "never outside the folder");
        #[cfg(windows)]
        {
            let mut mark = a.as_os_str().to_owned();
            mark.push(":Zone.Identifier");
            let zone = fs::read_to_string(mark).unwrap();
            assert!(zone.contains("ZoneId=3"), "{zone}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn attachments_come_from_the_stored_message() {
        let store = crate::sample::sample_store();
        // The sample mailbox has no raw messages: asking for an attachment says so.
        let id = store.list("firma/inbox").unwrap()[0].id;
        assert!(store.attachment(id, 0).is_err());
        let dir = temp_dir("opened");
        assert!(opened_dir(&dir, 42).ends_with(Path::new("Telex").join("42")));
        clear_opened(&dir);
    }
}
