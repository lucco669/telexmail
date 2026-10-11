//! Mail in and out of Telex as files: a folder written as an mbox file, the format other mail
//! programs import, and .eml and mbox files read into a folder on this PC: the way in from
//! Thunderbird, The Bat! and the rest, and the way back out.
//!
//! mbox (mboxrd): each message starts with a "From " line (sender and date); a line of the
//! message that starts with "From " after any number of ">" gets one more ">", which reading
//! takes off again. The read state and flag travel in "Status:" and "X-Status:" headers, as
//! mutt and others write them; Thunderbird's "X-Mozilla-Status" is read too. Line ends are
//! LF in the file and CRLF in the mail base, as IMAP wants them.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::NaiveDateTime;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::mime::parse_message;
use crate::model::split_folder_key;
use crate::store::{insert_message, MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// How many messages go into the mail base in one transaction while importing. The store is
/// free for the window between them.
const IMPORT_BATCH: usize = 200;

/// One message read from a file: as it was, with CRLF line ends, and what its headers said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMessage {
    pub raw: Vec<u8>,
    pub seen: bool,
    pub flagged: bool,
    /// The date of mbox's "From " line, for a message without a Date header.
    pub date: Option<NaiveDateTime>,
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    /// Messages now in the folder.
    pub added: usize,
    /// Messages that were there already (the same Message-ID).
    pub already: usize,
    /// One line per file that couldn't be read: its name and why.
    pub problems: Vec<String>,
}

/// Progress while importing: messages read so far, and the file being read.
#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub done: usize,
    pub file: String,
}

impl MailStore {
    /// Write a folder's messages, oldest first, as mbox. Returns how many.
    pub fn export_mbox(&self, folder: &str, out: &mut dyn Write) -> Result<usize, StoreError> {
        write_mbox(&self.db, folder, out)
    }

    /// The Message-IDs already in a folder, so an import doesn't store a message twice.
    fn message_ids_in(&self, folder: &str) -> Result<HashSet<String>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT message_id FROM messages WHERE folder = ?1 AND message_id IS NOT NULL",
        )?;
        let ids = stmt
            .query_map([folder], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// Store messages read from files in a folder on this PC, skipping those whose Message-ID
    /// is in `known` (which grows with every message added). Returns (added, already there).
    pub fn import_messages(
        &mut self,
        folder: &str,
        batch: &[FileMessage],
        known: &mut HashSet<String>,
    ) -> Result<(usize, usize), StoreError> {
        check_local(self, folder)?;
        let (mut added, mut already) = (0, 0);
        let tx = self.db.transaction()?;
        for m in batch {
            let mut message = parse_message(&m.raw, folder, m.date);
            if let Some(id) = &message.message_id {
                if !known.insert(id.clone()) {
                    already += 1;
                    continue;
                }
            }
            message.unread = !m.seen;
            message.flagged = m.flagged;
            insert_message(&tx, folder, None, &message, None, Some(&m.raw))?;
            added += 1;
        }
        tx.commit()?;
        Ok((added, already))
    }
}

/// Mail comes in only to folders on this PC: a server folder would need it uploaded.
fn check_local(store: &MailStore, folder: &str) -> Result<(), StoreError> {
    match split_folder_key(folder) {
        Some((LOCAL_ID, _)) if store.kind_of(folder).is_some() => Ok(()),
        Some((LOCAL_ID, _)) => Err(StoreError::UnknownFolder(folder.to_owned())),
        _ => Err(StoreError::Invalid(tr!(
            "Mail from files goes into folders on this PC.",
            "Poczta z plików trafia do folderów na tym komputerze."
        ))),
    }
}

/// Export `folder` of the mail base at `db_path` to the file `dest` as mbox, on a connection
/// of its own, so the window and checks go on meanwhile. The file appears only when it is
/// complete. Returns how many messages it holds.
pub fn export_folder(db_path: &Path, folder: &str, dest: &Path) -> Result<usize, StoreError> {
    let db = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let part = with_suffix(dest, ".part");
    let written = (|| {
        let mut out = BufWriter::new(File::create(&part).map_err(file_error)?);
        let n = write_mbox(&db, folder, &mut out)?;
        out.flush().map_err(file_error)?;
        Ok(n)
    })();
    match written {
        Ok(n) => {
            std::fs::rename(&part, dest).map_err(file_error)?;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// `report.mbox` + ".part" → `report.mbox.part`.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn file_error(e: std::io::Error) -> StoreError {
    StoreError::Invalid(tr!(
        "Telex couldn't write the file: {e}.",
        "Telex nie mógł zapisać pliku: {e}."
    ))
}

/// The messages of `folder`, oldest first, as mbox.
fn write_mbox(db: &Connection, folder: &str, out: &mut dyn Write) -> Result<usize, StoreError> {
    let messages: Vec<(i64, String, NaiveDateTime, bool, bool)> = {
        let mut stmt = db.prepare(
            "SELECT id, from_email, date, unread, flagged FROM messages
             WHERE folder = ?1 ORDER BY date, id",
        )?;
        let rows = stmt
            .query_map([folder], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        rows
    };
    let mut raw_of = db.prepare("SELECT raw FROM message_content WHERE id = ?1")?;
    let mut written = 0;
    for (id, from, date, unread, flagged) in messages {
        let raw: Option<Vec<u8>> = raw_of.query_row([id], |r| r.get(0)).ok().flatten();
        let Some(raw) = raw else { continue }; // nothing to write
        let sender = if from.trim().is_empty() || from.contains(char::is_whitespace) {
            "MAILER-DAEMON"
        } else {
            from.trim()
        };
        let mut text =
            format!("From {sender} {}\n", date.format("%a %b %e %H:%M:%S %Y")).into_bytes();
        text.extend_from_slice(if unread {
            b"Status: O\n"
        } else {
            b"Status: RO\n"
        });
        if flagged {
            text.extend_from_slice(b"X-Status: F\n");
        }
        let mut in_headers = true;
        for line in lines(&raw) {
            if in_headers {
                if line.is_empty() {
                    in_headers = false;
                } else if is_status_header(line) {
                    continue; // ours above say it now
                }
            }
            if line
                .iter()
                .skip_while(|&&b| b == b'>')
                .take(5)
                .eq(b"From ".iter())
            {
                text.push(b'>');
            }
            text.extend_from_slice(line);
            text.push(b'\n');
        }
        text.push(b'\n');
        out.write_all(&text).map_err(file_error)?;
        written += 1;
    }
    Ok(written)
}

/// The lines of a message without their line ends, CRLF or LF.
fn lines(raw: &[u8]) -> impl Iterator<Item = &[u8]> {
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    raw.split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

fn header_name_is(line: &[u8], name: &str) -> bool {
    line.len() > name.len()
        && line[name.len()] == b':'
        && line[..name.len()].eq_ignore_ascii_case(name.as_bytes())
}

fn is_status_header(line: &[u8]) -> bool {
    [
        "Status",
        "X-Status",
        "X-Mozilla-Status",
        "X-Mozilla-Status2",
    ]
    .iter()
    .any(|h| header_name_is(line, h))
}

/// Read state and flag from a message's own headers. Without any, mail from a file counts as
/// read: it is usually an archive.
fn flags_of(raw: &[u8]) -> (bool, bool) {
    let (mut seen, mut flagged) = (None, false);
    for line in lines(raw).take_while(|l| !l.is_empty()) {
        let value = |name: &str| {
            header_name_is(line, name).then(|| {
                String::from_utf8_lossy(&line[name.len() + 1..])
                    .trim()
                    .to_owned()
            })
        };
        if let Some(v) = value("Status") {
            seen = Some(v.contains('R'));
        } else if let Some(v) = value("X-Status") {
            flagged |= v.contains('F');
        } else if let Some(v) = value("X-Mozilla-Status") {
            if let Ok(bits) = u32::from_str_radix(&v, 16) {
                seen = Some(bits & 0x1 != 0);
                flagged |= bits & 0x4 != 0;
            }
        }
    }
    (seen.unwrap_or(true), flagged)
}

/// The date of an mbox "From " line: "From anna@x.pl Sat Oct 10 22:35:00 2026".
fn from_line_date(line: &[u8]) -> Option<NaiveDateTime> {
    let text = String::from_utf8_lossy(line);
    let words: Vec<&str> = text.split_whitespace().collect();
    let tail = words.get(words.len().checked_sub(5)?..)?.join(" ");
    NaiveDateTime::parse_from_str(&tail, "%a %b %e %H:%M:%S %Y").ok()
}

/// One finished message: CRLF line ends, the blank line before the next "From " left out.
fn finish(mut lines: Vec<Vec<u8>>, date: Option<NaiveDateTime>) -> Option<FileMessage> {
    while lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    if lines.is_empty() {
        return None;
    }
    let mut raw = lines.join(&b"\r\n"[..]);
    raw.extend_from_slice(b"\r\n");
    let (seen, flagged) = flags_of(&raw);
    Some(FileMessage {
        raw,
        seen,
        flagged,
        date,
    })
}

/// Read the messages in a file, giving each to `each`: an mbox file holds many, anything else
/// (.eml) one. Returns how many there were.
pub fn read_file(
    path: &Path,
    each: &mut dyn FnMut(FileMessage) -> Result<(), StoreError>,
) -> Result<usize, StoreError> {
    let unreadable = |e: std::io::Error| {
        StoreError::Invalid(tr!("can't read it: {e}", "nie można go odczytać: {e}"))
    };
    let mut reader = BufReader::new(File::open(path).map_err(unreadable)?);
    let mbox = reader.fill_buf().map_err(unreadable)?.starts_with(b"From ");
    if !mbox {
        let mut raw = Vec::new();
        reader.read_to_end(&mut raw).map_err(unreadable)?;
        let lines: Vec<Vec<u8>> = lines(&raw).map(<[u8]>::to_vec).collect();
        return match finish(lines, None) {
            Some(m) => each(m).map(|()| 1),
            None => Ok(0),
        };
    }
    let mut count = 0;
    let mut current: Option<(Vec<Vec<u8>>, Option<NaiveDateTime>)> = None;
    let mut blank = true;
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).map_err(unreadable)? == 0 {
            break;
        }
        let text = line.strip_suffix(b"\n").unwrap_or(&line);
        let text = text.strip_suffix(b"\r").unwrap_or(text);
        if blank && text.starts_with(b"From ") {
            if let Some(m) = current.take().and_then(|(lines, date)| finish(lines, date)) {
                each(m)?;
                count += 1;
            }
            current = Some((Vec::new(), from_line_date(text)));
            blank = false;
            continue;
        }
        blank = text.is_empty();
        // mboxrd: ">From " was "From ", ">>From " was ">From ".
        let quoted = text.iter().take_while(|&&b| b == b'>').count();
        let text = if quoted > 0 && text[quoted..].starts_with(b"From ") {
            &text[1..]
        } else {
            text
        };
        if let Some((lines, _)) = &mut current {
            lines.push(text.to_vec());
        }
    }
    if let Some(m) = current.and_then(|(lines, date)| finish(lines, date)) {
        each(m)?;
        count += 1;
    }
    Ok(count)
}

/// Read .eml and mbox files into a folder on this PC, a batch at a time so the window keeps
/// working, skipping messages that are there already. A file that can't be read is reported
/// and the others still come in.
pub fn import_files(
    store: &Mutex<MailStore>,
    folder: &str,
    paths: &[PathBuf],
    progress: &(dyn Fn(ImportProgress) + Send + Sync),
) -> Result<ImportReport, StoreError> {
    let lock = || store.lock().map_err(|_| StoreError::poisoned());
    let mut known = {
        let s = lock()?;
        check_local(&s, folder)?;
        s.message_ids_in(folder)?
    };
    let mut report = ImportReport::default();
    let mut done = 0;
    for path in paths {
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let mut batch = Vec::new();
        let mut flush =
            |batch: &mut Vec<FileMessage>, report: &mut ImportReport, done: &mut usize| {
                if batch.is_empty() {
                    return Ok(());
                }
                let (added, already) = lock()?.import_messages(folder, batch, &mut known)?;
                report.added += added;
                report.already += already;
                *done += batch.len();
                batch.clear();
                progress(ImportProgress {
                    done: *done,
                    file: file.clone(),
                });
                Ok::<(), StoreError>(())
            };
        let read = read_file(path, &mut |m| {
            batch.push(m);
            if batch.len() >= IMPORT_BATCH {
                flush(&mut batch, &mut report, &mut done)?;
            }
            Ok(())
        });
        flush(&mut batch, &mut report, &mut done)?;
        match read {
            Ok(_) => {}
            Err(StoreError::Invalid(why)) => report.problems.push(format!("{file}: {why}")),
            Err(e) => return Err(e),
        }
    }
    Ok(report)
}

/// The mbox file's name for a folder: its name, without characters Windows doesn't allow.
pub fn file_name_for(folder_name: &str) -> String {
    let clean: String = folder_name
        .chars()
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.');
    format!("{}.mbox", if clean.is_empty() { "mail" } else { clean })
}

/// For tests: the raw text of a message as `import` would store it.
#[cfg(test)]
fn crlf(text: &str) -> Vec<u8> {
    text.replace('\n', "\r\n").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Folder, FolderKind};
    use crate::store::SyncedMessage;

    fn store() -> (MailStore, String) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        store
            .set_folders(
                &account.id,
                &[Folder {
                    id: "inbox".into(),
                    name: "Inbox".into(),
                    kind: FolderKind::Inbox,
                    remote: Some("INBOX".into()),
                }],
            )
            .unwrap();
        (store, account.folder_key("inbox"))
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "telex-mailbox-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    const ANNA: &str = "From: Anna Kowalska <anna@x.pl>\nTo: biuro@twojastudio.pl\nSubject: Wycena\nMessage-ID: <w1@x.pl>\nDate: Fri, 09 Oct 2026 14:30:00 +0200\n\nDzień dobry,\nFrom now on the price is lower.\n>From the old list.\n";

    #[test]
    fn a_folder_goes_out_as_mbox_and_comes_back_the_same() {
        let (mut store, inbox) = store();
        let mut first = crate::mime::parse_message(&crlf(ANNA), &inbox, None);
        first.unread = false;
        first.flagged = true;
        let second_raw = crlf("From: Piotr <piotr@y.pl>\nSubject: Termin\nMessage-ID: <t2@y.pl>\nDate: Sat, 10 Oct 2026 08:00:00 +0200\nStatus: O\n\nCzy wtorek?\n");
        let second = crate::mime::parse_message(&second_raw, &inbox, None);
        store
            .add_synced(
                &inbox,
                &[
                    SyncedMessage {
                        uid: 1,
                        message: first,
                        raw: crlf(ANNA),
                    },
                    SyncedMessage {
                        uid: 2,
                        message: second,
                        raw: second_raw,
                    },
                ],
            )
            .unwrap();
        let mut out = Vec::new();
        assert_eq!(store.export_mbox(&inbox, &mut out).unwrap(), 2);
        let text = String::from_utf8(out.clone()).unwrap();
        assert!(text.starts_with("From anna@x.pl Fri Oct  9 "), "{text}");
        assert!(
            text.contains("Status: RO\nX-Status: F\nFrom: Anna"),
            "our flags first"
        );
        assert!(
            text.contains("\n>From now on"),
            "body lines that start with From are quoted"
        );
        assert!(text.contains("\n>>From the old list."));
        assert!(!text.contains('\r'), "LF in the file");
        assert_eq!(
            text.lines().filter(|l| l.starts_with("Status:")).count(),
            2,
            "the old Status header is replaced"
        );
        assert!(text.contains("\n\nFrom piotr@y.pl Sat Oct 10 "));

        // Back in, into a folder on this PC: the same text, flags and dates.
        let path = temp("Skrzynka.mbox");
        std::fs::write(&path, &out).unwrap();
        let mut read = Vec::new();
        assert_eq!(
            read_file(&path, &mut |m| {
                read.push(m);
                Ok(())
            })
            .unwrap(),
            2
        );
        assert!((read[0].seen, read[0].flagged) == (true, true));
        assert!((read[1].seen, read[1].flagged) == (false, false));
        let body = String::from_utf8(read[0].raw.clone()).unwrap();
        assert!(
            body.ends_with(
                "Dzień dobry,\r\nFrom now on the price is lower.\r\n>From the old list.\r\n"
            ),
            "{body:?}"
        );

        let store = Mutex::new(store);
        let report = import_files(
            &store,
            "local/archive",
            std::slice::from_ref(&path),
            &|_| {},
        )
        .unwrap();
        assert_eq!((report.added, report.already), (2, 0));
        let again = import_files(
            &store,
            "local/archive",
            std::slice::from_ref(&path),
            &|_| {},
        )
        .unwrap();
        assert_eq!((again.added, again.already), (0, 2), "nothing twice");
        let s = store.lock().unwrap();
        let archived = s.list("local/archive").unwrap();
        assert_eq!(archived.len(), 2);
        let wycena = archived.iter().find(|m| m.subject == "Wycena").unwrap();
        assert!(!wycena.unread && wycena.flagged);
        assert_eq!(wycena.date.format("%Y-%m-%d").to_string(), "2026-10-09");
        assert_eq!(
            s.search("wtorek").unwrap().len(),
            2,
            "searchable at once, as the Inbox copy is"
        );
        assert_eq!(
            s.raw_of(wycena.id)
                .unwrap()
                .unwrap()
                .iter()
                .filter(|&&b| b == b'\n')
                .count(),
            s.raw_of(wycena.id)
                .unwrap()
                .unwrap()
                .windows(2)
                .filter(|w| w == b"\r\n")
                .count(),
            "every line ends in CRLF, as IMAP wants it"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn eml_files_and_thunderbird_flags_come_in() {
        let (store, _) = store();
        let eml = temp("wycena.eml");
        std::fs::write(&eml, ANNA).unwrap();
        let tb = temp("Inbox");
        std::fs::write(
            &tb,
            "From - Sat Oct 10 22:35:00 2026\nX-Mozilla-Status: 0005\nSubject: Z Thunderbirda\nMessage-ID: <tb@z.pl>\n\nTreść\n\nFrom - Sun Oct 11 09:00:00 2026\nX-Mozilla-Status: 0000\nSubject: Bez daty\n\nDruga\n",
        )
        .unwrap();
        let missing = temp("nie-ma.mbox");
        let store = Mutex::new(store);
        let report = import_files(
            &store,
            "local/archive",
            &[eml.clone(), tb.clone(), missing.clone()],
            &|_| {},
        )
        .unwrap();
        assert_eq!(report.added, 3);
        assert_eq!(report.problems.len(), 1, "{:?}", report.problems);
        assert!(report.problems[0].starts_with("nie-ma.mbox: "));
        let s = store.lock().unwrap();
        let all = s.list("local/archive").unwrap();
        let tb_mail = all.iter().find(|m| m.subject == "Z Thunderbirda").unwrap();
        assert!(
            !tb_mail.unread && tb_mail.flagged,
            "read and starred in Thunderbird"
        );
        let undated = all.iter().find(|m| m.subject == "Bez daty").unwrap();
        assert!(undated.unread);
        assert_eq!(
            undated.date.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-11 09:00",
            "the From line's date"
        );
        let eml_mail = all.iter().find(|m| m.subject == "Wycena").unwrap();
        assert!(!eml_mail.unread, "an .eml without Status counts as read");
        // Only into folders on this PC.
        drop(s);
        assert!(import_files(
            &store,
            "twojastudio/inbox",
            std::slice::from_ref(&eml),
            &|_| {}
        )
        .is_err());
        let _ = std::fs::remove_dir_all(eml.parent().unwrap());
        let _ = std::fs::remove_dir_all(tb.parent().unwrap());
    }

    #[test]
    fn exported_files_are_whole_and_named_safely() {
        let (store, inbox) = store();
        let path = temp("telex.db");
        drop(store);
        // A mail base on disk, read on a connection of its own.
        let mut on_disk = MailStore::open(&path).unwrap();
        on_disk
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        let key = on_disk.create_local_folder("Faktury").unwrap();
        let held = Mutex::new(on_disk);
        let eml = temp("f.eml");
        std::fs::write(&eml, ANNA).unwrap();
        import_files(&held, &key, std::slice::from_ref(&eml), &|_| {}).unwrap();
        let dest = path.with_file_name(file_name_for("Faktury: 2026/10?"));
        assert_eq!(dest.file_name().unwrap(), "Faktury- 2026-10-.mbox");
        assert_eq!(export_folder(&path, &key, &dest).unwrap(), 1);
        assert!(std::fs::read_to_string(&dest)
            .unwrap()
            .contains("Subject: Wycena"));
        assert!(!with_suffix(&dest, ".part").exists());
        assert_eq!(file_name_for(" .. "), "mail.mbox");
        let _ = inbox;
        drop(held);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let _ = std::fs::remove_dir_all(eml.parent().unwrap());
    }
}
