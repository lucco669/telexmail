//! Mail in and out of Telex as files: a folder written as an mbox file, the format other mail
//! programs import, and mail files read into a folder on this PC: the way in from Thunderbird,
//! The Bat!, Outlook, Apple Mail and the rest, and the way back out.
//!
//! mbox (mboxrd): each message starts with a "From " line (sender and date); a line of the
//! message that starts with "From " after any number of ">" gets one more ">", which reading
//! takes off again. The read state and flag travel in "Status:" and "X-Status:" headers, as
//! mutt and others write them; Thunderbird's "X-Mozilla-Status" is read too, and a message it
//! deleted but hasn't compacted away yet stays out. Tags travel as keywords: read from
//! Thunderbird's "X-Mozilla-Keys" and from "X-Keywords" (Dovecot, mutt), the keywords that are
//! tags here becoming the message's tags; written as "X-Keywords". Line ends are LF in the
//! file and CRLF in the mail base, as IMAP wants them.
//!
//! Besides mbox: .eml (one message), .emlx (Apple Mail), .msg (Outlook, see
//! `migrate::outlook`), and folders in Maildir (Evolution, KMail, mutt) or MH (Claws Mail)
//! form, a file per message.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Local, NaiveDateTime};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::mime::parse_message;
use crate::model::{split_folder_key, Message};
use crate::store::{insert_message, MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// How many messages go into the mail base in one transaction while importing. The store is
/// free for the window between them.
const IMPORT_BATCH: usize = 200;
/// A batch is stored sooner when its messages reach this size, so big mail isn't all in memory.
const IMPORT_BATCH_BYTES: usize = 32 * 1024 * 1024;
/// Bytes read from a file at a time: big mbox files go faster in large pieces.
const READ_BUFFER: usize = 1 << 20;

/// One message read from a file: as it was, with CRLF line ends, and what its headers (or
/// its file's name) said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMessage {
    pub raw: Vec<u8>,
    pub seen: bool,
    pub flagged: bool,
    /// The date of mbox's "From " line, or the file's, for a message without a Date header.
    pub date: Option<NaiveDateTime>,
    /// IMAP keywords its headers carry (`X-Mozilla-Keys`, `X-Keywords`), lowercase: the ones
    /// that are tags here become its tags.
    pub keywords: Vec<String>,
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

impl ImportReport {
    pub fn add(&mut self, other: ImportReport) {
        self.added += other.added;
        self.already += other.already;
        self.problems.extend(other.problems);
    }
}

/// Progress while importing: messages and bytes read so far, and the file being read.
#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub done: usize,
    pub bytes: u64,
    pub file: String,
}

impl MailStore {
    /// Write a folder's messages, oldest first, as mbox. Returns how many. (The app writes
    /// a file on a connection of its own: `export_folder`.)
    #[cfg(test)]
    pub(crate) fn export_mbox(
        &self,
        folder: &str,
        out: &mut dyn Write,
    ) -> Result<usize, StoreError> {
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

    /// Store messages read from files (and parsed: `parsed[i]` is `files[i]`) in a folder on
    /// this PC, skipping those whose Message-ID is in `known`, which grows with every message
    /// added. Returns (added, already there).
    fn store_imported(
        &mut self,
        folder: &str,
        parsed: &[Message],
        files: &[FileMessage],
        known: &mut HashSet<String>,
    ) -> Result<(usize, usize), StoreError> {
        check_local(self, folder)?;
        let (mut added, mut already) = (0, 0);
        let tx = self.db.transaction()?;
        for (message, file) in parsed.iter().zip(files) {
            if let Some(id) = &message.message_id {
                if !known.insert(id.clone()) {
                    already += 1;
                    continue;
                }
            }
            insert_message(&tx, folder, None, message, None, Some(&file.raw))?;
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
    type Row = (i64, String, NaiveDateTime, bool, bool, String);
    let messages: Vec<Row> = {
        let mut stmt = db.prepare(
            "SELECT id, from_email, date, unread, flagged, keywords FROM messages
             WHERE folder = ?1 ORDER BY date, id",
        )?;
        let rows = stmt
            .query_map([folder], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        rows
    };
    let mut raw_of = db.prepare("SELECT raw FROM message_content WHERE id = ?1")?;
    let mut written = 0;
    for (id, from, date, unread, flagged, keywords) in messages {
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
        // Its tags, as Dovecot and mutt keep keywords.
        if !keywords.is_empty() {
            text.extend_from_slice(format!("X-Keywords: {keywords}\n").as_bytes());
        }
        let (mut in_headers, mut dropping) = (true, false);
        for line in lines(&raw) {
            if in_headers {
                if line.is_empty() {
                    in_headers = false;
                } else if dropping && line.first().is_some_and(|b| *b == b' ' || *b == b'\t') {
                    continue; // the rest of a header left out
                } else if is_status_header(line) {
                    dropping = true;
                    continue; // ours above say it now
                } else {
                    dropping = false;
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
        "X-Mozilla-Keys",
        "X-Keywords",
    ]
    .iter()
    .any(|h| header_name_is(line, h))
}

/// Thunderbird's X-Mozilla-Status bits: read, flagged ("marked"), and deleted but still in
/// the file until the folder is compacted.
const MOZ_READ: u32 = 0x0001;
const MOZ_MARKED: u32 = 0x0004;
const MOZ_EXPUNGED: u32 = 0x0008;

/// What a message's own headers say about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct HeaderFlags {
    seen: Option<bool>,
    flagged: bool,
    /// Deleted in Thunderbird: it stays out.
    deleted: bool,
    /// Thunderbird's `X-Mozilla-Keys` and Dovecot's or mutt's `X-Keywords`: keywords apart by
    /// spaces (or commas), lowercase, each once.
    keywords: Vec<String>,
}

/// The headers that carry keywords.
const KEYWORD_HEADERS: [&str; 2] = ["X-Mozilla-Keys", "X-Keywords"];

fn header_flags(raw: &[u8]) -> HeaderFlags {
    let mut flags = HeaderFlags::default();
    let mut in_keywords = false;
    for line in lines(raw).take_while(|l| !l.is_empty()) {
        let folded = line.first().is_some_and(|b| *b == b' ' || *b == b'\t');
        let keywords = if folded {
            in_keywords.then_some(line)
        } else {
            in_keywords = false;
            KEYWORD_HEADERS
                .iter()
                .find(|h| header_name_is(line, h))
                .map(|h| &line[h.len() + 1..])
        };
        if let Some(text) = keywords {
            in_keywords = true;
            for k in String::from_utf8_lossy(text).split([' ', '\t', ',']) {
                let k = k.to_ascii_lowercase();
                if crate::tags::is_keyword(&k) && !flags.keywords.contains(&k) {
                    flags.keywords.push(k);
                }
            }
            continue;
        }
        let value = |name: &str| {
            header_name_is(line, name).then(|| {
                String::from_utf8_lossy(&line[name.len() + 1..])
                    .trim()
                    .to_owned()
            })
        };
        if let Some(v) = value("Status") {
            flags.seen = Some(v.contains('R'));
        } else if let Some(v) = value("X-Status") {
            flags.flagged |= v.contains('F');
        } else if let Some(v) = value("X-Mozilla-Status") {
            if let Ok(bits) = u32::from_str_radix(&v, 16) {
                flags.seen = Some(bits & MOZ_READ != 0);
                flags.flagged |= bits & MOZ_MARKED != 0;
                flags.deleted = bits & MOZ_EXPUNGED != 0;
            }
        }
    }
    flags
}

/// The date of an mbox "From " line: "From anna@x.pl Sat Oct 10 22:35:00 2026".
fn from_line_date(line: &[u8]) -> Option<NaiveDateTime> {
    let text = String::from_utf8_lossy(line);
    let words: Vec<&str> = text.split_whitespace().collect();
    let tail = words.get(words.len().checked_sub(5)?..)?.join(" ");
    NaiveDateTime::parse_from_str(&tail, "%a %b %e %H:%M:%S %Y").ok()
}

/// One finished message: the blank lines before the next "From " left out, its flags read.
/// Without any, mail from a file counts as read: it is usually an archive. `None` for an
/// empty one, and for one Thunderbird deleted.
fn finish(mut raw: Vec<u8>, date: Option<NaiveDateTime>) -> Option<FileMessage> {
    while raw.ends_with(b"\r\n\r\n") {
        raw.truncate(raw.len() - 2);
    }
    if raw.is_empty() || raw == b"\r\n" {
        return None;
    }
    let flags = header_flags(&raw);
    (!flags.deleted).then_some(FileMessage {
        raw,
        seen: flags.seen.unwrap_or(true),
        flagged: flags.flagged,
        date,
        keywords: flags.keywords,
    })
}

/// `bytes` with every line ending in CRLF.
pub(crate) fn crlf_lines(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 32);
    for line in lines(bytes) {
        out.extend_from_slice(line);
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// When a file was last changed, for a message without a Date header.
fn file_date(path: &Path) -> Option<NaiveDateTime> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(DateTime::<Local>::from(modified).naive_local())
}

fn unreadable(e: std::io::Error) -> StoreError {
    StoreError::Invalid(tr!("can't read it: {e}", "nie można go odczytać: {e}"))
}

/// Read the messages at `path`, giving each to `each`: an mbox file holds many, a Maildir or
/// MH folder a file each, anything else one (.eml, .emlx, .msg). Returns how many there were.
pub fn read_path(
    path: &Path,
    each: &mut dyn FnMut(FileMessage) -> Result<(), StoreError>,
) -> Result<usize, StoreError> {
    if path.is_dir() {
        return read_mail_dir(path, each);
    }
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("msg") => {
            let m = crate::migrate::outlook::read_msg(path)?;
            each(m).map(|()| 1)
        }
        Some("emlx") => read_emlx(path, each),
        _ => read_file(path, each),
    }
}

/// An mbox file, or a file that holds one message (.eml).
fn read_file(
    path: &Path,
    each: &mut dyn FnMut(FileMessage) -> Result<(), StoreError>,
) -> Result<usize, StoreError> {
    let mut reader = BufReader::with_capacity(READ_BUFFER, File::open(path).map_err(unreadable)?);
    let mbox = reader.fill_buf().map_err(unreadable)?.starts_with(b"From ");
    if !mbox {
        let mut raw = Vec::new();
        reader.read_to_end(&mut raw).map_err(unreadable)?;
        return match finish(crlf_lines(&raw), file_date(path)) {
            Some(m) => each(m).map(|()| 1),
            None => Ok(0),
        };
    }
    let mut count = 0;
    let mut current: Option<(Vec<u8>, Option<NaiveDateTime>)> = None;
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
            if let Some(m) = current.take().and_then(|(raw, date)| finish(raw, date)) {
                each(m)?;
                count += 1;
            }
            current = Some((Vec::with_capacity(16 * 1024), from_line_date(text)));
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
        if let Some((raw, _)) = &mut current {
            raw.extend_from_slice(text);
            raw.extend_from_slice(b"\r\n");
        }
    }
    if let Some(m) = current.and_then(|(raw, date)| finish(raw, date)) {
        each(m)?;
        count += 1;
    }
    Ok(count)
}

/// Apple Mail's .emlx: the message's length on the first line, the message, then a property
/// list whose "flags" say read (bit 0), deleted (bit 1) and flagged (bit 4).
fn read_emlx(
    path: &Path,
    each: &mut dyn FnMut(FileMessage) -> Result<(), StoreError>,
) -> Result<usize, StoreError> {
    let bytes = std::fs::read(path).map_err(unreadable)?;
    let start = bytes.iter().position(|&b| b == b'\n').map_or(0, |i| i + 1);
    let length = std::str::from_utf8(&bytes[..start])
        .ok()
        .and_then(|l| l.trim().parse::<usize>().ok());
    let Some(end) = length
        .and_then(|n| start.checked_add(n))
        .filter(|&end| end <= bytes.len())
    else {
        return read_file(path, each); // not an .emlx after all
    };
    let plist = String::from_utf8_lossy(&bytes[end..]);
    let flags = plist
        .split_once("<key>flags</key>")
        .and_then(|(_, rest)| rest.split_once("<integer>"))
        .and_then(|(_, rest)| rest.split_once("</integer>"))
        .and_then(|(n, _)| n.trim().parse::<u64>().ok())
        .unwrap_or(1);
    if flags & 0b10 != 0 {
        return Ok(0);
    }
    match finish(crlf_lines(&bytes[start..end]), file_date(path)) {
        Some(mut m) => {
            m.seen = flags & 1 != 0;
            m.flagged = flags & 0x10 != 0;
            each(m).map(|()| 1)
        }
        None => Ok(0),
    }
}

/// Whether a folder holds mail as a file per message: Maildir (cur/ and new/) or MH (files
/// named 1, 2, 3…).
pub fn is_mail_dir(path: &Path) -> bool {
    path.join("cur").is_dir() || path.join("new").is_dir() || !mh_files(path).is_empty()
}

/// An MH folder's messages, in order.
fn mh_files(path: &Path) -> Vec<(u32, PathBuf)> {
    let mut files: Vec<(u32, PathBuf)> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_str()?.parse::<u32>().ok()?;
            e.file_type().ok()?.is_file().then(|| (n, e.path()))
        })
        .collect();
    files.sort_unstable_by_key(|(n, _)| *n);
    files
}

/// MH's ".mh_sequences": "unseen: 1-3 7" → {1, 2, 3, 7}.
fn mh_sequence(path: &Path, name: &str) -> Option<HashSet<u32>> {
    let text = std::fs::read_to_string(path.join(".mh_sequences")).ok()?;
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))?;
    let mut set = HashSet::new();
    for part in line.split_whitespace() {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                    set.extend(a..=b.min(a.saturating_add(1_000_000)));
                }
            }
            None => set.extend(part.parse::<u32>().ok()),
        }
    }
    Some(set)
}

/// A Maildir or MH folder: a file per message. Maildir keeps the state in the file's name
/// (":2," then S seen, F flagged, T deleted); mail in new/ is unread.
fn read_mail_dir(
    path: &Path,
    each: &mut dyn FnMut(FileMessage) -> Result<(), StoreError>,
) -> Result<usize, StoreError> {
    let mut count = 0;
    let mut one = |file: &Path, state: Option<(bool, bool)>| -> Result<(), StoreError> {
        let Ok(bytes) = std::fs::read(file) else {
            return Ok(()); // gone meanwhile, or not ours to read
        };
        if let Some(mut m) = finish(crlf_lines(&bytes), file_date(file)) {
            if let Some((seen, flagged)) = state {
                (m.seen, m.flagged) = (seen, flagged);
            }
            each(m)?;
            count += 1;
        }
        Ok(())
    };
    if path.join("cur").is_dir() || path.join("new").is_dir() {
        for (sub, new) in [("cur", false), ("new", true)] {
            let mut files: Vec<PathBuf> = std::fs::read_dir(path.join(sub))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .map(|e| e.path())
                .collect();
            files.sort_unstable();
            for file in files {
                let name = file.file_name().unwrap_or_default().to_string_lossy();
                // Windows can't have ":" in a name; Maildir copies made there use "!" or ";".
                let info = name
                    .rsplit_once(":2,")
                    .or_else(|| name.rsplit_once("!2,"))
                    .or_else(|| name.rsplit_once(";2,"))
                    .map_or("", |(_, i)| i);
                if info.contains('T') {
                    continue;
                }
                let state = (!new && info.contains('S'), info.contains('F'));
                one(&file, Some(state))?;
            }
        }
        return Ok(count);
    }
    let unseen = mh_sequence(path, "unseen");
    let flagged = mh_sequence(path, "flagged").unwrap_or_default();
    for (n, file) in mh_files(path) {
        let state = unseen
            .as_ref()
            .map(|u| (!u.contains(&n), flagged.contains(&n)));
        one(&file, state)?;
    }
    Ok(count)
}

/// Reads mail files into one folder on this PC, a batch at a time so the window keeps
/// working, skipping messages that are there already. Messages are parsed before the store
/// is locked; only storing them holds it.
pub(crate) struct Importer<'a> {
    store: &'a Mutex<MailStore>,
    folder: String,
    known: HashSet<String>,
    /// The keywords that are tags here: only those of a message's keywords come along.
    tags: HashSet<String>,
    batch: Vec<FileMessage>,
    batch_bytes: usize,
    progress: &'a (dyn Fn(ImportProgress) + Send + Sync),
    file: String,
    pub report: ImportReport,
    /// Messages and bytes read so far, counted from where the caller's progress starts.
    pub done: usize,
    pub bytes: u64,
}

impl<'a> Importer<'a> {
    pub fn new(
        store: &'a Mutex<MailStore>,
        folder: &str,
        progress: &'a (dyn Fn(ImportProgress) + Send + Sync),
    ) -> Result<Self, StoreError> {
        let (known, tags) = {
            let s = store.lock().map_err(|_| StoreError::poisoned())?;
            check_local(&s, folder)?;
            (s.message_ids_in(folder)?, s.tag_keywords()?)
        };
        Ok(Self {
            store,
            folder: folder.to_owned(),
            known,
            tags,
            batch: Vec::with_capacity(IMPORT_BATCH),
            batch_bytes: 0,
            progress,
            file: String::new(),
            report: ImportReport::default(),
            done: 0,
            bytes: 0,
        })
    }

    /// Read a file or a mail folder. One that can't be read is reported by `name`, and the
    /// rest still comes in.
    pub fn read(&mut self, path: &Path, name: &str) -> Result<(), StoreError> {
        name.clone_into(&mut self.file);
        let read = read_path(path, &mut |m| self.add(m));
        self.flush()?;
        match read {
            Ok(_) => Ok(()),
            Err(StoreError::Invalid(why)) => {
                self.report.problems.push(format!("{name}: {why}"));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn add(&mut self, m: FileMessage) -> Result<(), StoreError> {
        self.batch_bytes += m.raw.len();
        self.batch.push(m);
        if self.batch.len() >= IMPORT_BATCH || self.batch_bytes >= IMPORT_BATCH_BYTES {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), StoreError> {
        if self.batch.is_empty() {
            return Ok(());
        }
        let batch = std::mem::take(&mut self.batch);
        self.batch_bytes = 0;
        let parsed: Vec<Message> = batch
            .iter()
            .map(|m| {
                let mut message = parse_message(&m.raw, &self.folder, m.date);
                message.unread = !m.seen;
                message.flagged = m.flagged;
                message.tags = crate::tags::only_tags(&m.keywords, &self.tags);
                message
            })
            .collect();
        let (added, already) = self
            .store
            .lock()
            .map_err(|_| StoreError::poisoned())?
            .store_imported(&self.folder, &parsed, &batch, &mut self.known)?;
        self.report.added += added;
        self.report.already += already;
        self.done += batch.len();
        self.bytes += batch.iter().map(|m| m.raw.len() as u64).sum::<u64>();
        (self.progress)(ImportProgress {
            done: self.done,
            bytes: self.bytes,
            file: self.file.clone(),
        });
        self.batch = batch;
        self.batch.clear();
        Ok(())
    }
}

/// Read mail files and folders into a folder on this PC, skipping messages that are there
/// already. A file that can't be read is reported and the others still come in.
pub fn import_files(
    store: &Mutex<MailStore>,
    folder: &str,
    paths: &[PathBuf],
    progress: &(dyn Fn(ImportProgress) + Send + Sync),
) -> Result<ImportReport, StoreError> {
    let mut importer = Importer::new(store, folder, progress)?;
    for path in paths {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        importer.read(path, &name)?;
    }
    Ok(importer.report)
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
    fn keywords_that_are_tags_come_in_and_go_out_as_x_keywords() {
        let (mut store, _) = store();
        store
            .save_tag(&crate::tags::Tag {
                keyword: String::new(),
                name: "Projekt X".into(),
                color: "teal".into(),
                count: 0,
            })
            .unwrap();
        let path = temp("Inbox");
        std::fs::write(
            &path,
            "From - Sat Oct 10 22:35:00 2026\nX-Mozilla-Status: 0001\nX-Mozilla-Keys: Projekt_X $label1 NonJunk                                \nSubject: Thunderbird\nMessage-ID: <tb@x.pl>\n\nTreść\n\n\
             From MAILER-DAEMON Sun Oct 11 09:00:00 2026\nStatus: RO\nX-Keywords: $Forwarded,\n $label4\nSubject: Dovecot\nMessage-ID: <dc@x.pl>\n\nTreść\n\n\
             From - Mon Oct 12 09:00:00 2026\nSubject: Bez\nMessage-ID: <bez@x.pl>\n\nTreść\n",
        )
        .unwrap();
        let store = Mutex::new(store);
        let report = import_files(
            &store,
            "local/archive",
            std::slice::from_ref(&path),
            &|_| {},
        )
        .unwrap();
        assert_eq!(report.added, 3);
        let s = store.lock().unwrap();
        let all = s.list("local/archive").unwrap();
        let tags_of = |subject: &str| {
            let id = all.iter().find(|m| m.subject == subject).unwrap().id;
            s.keywords_of(id).unwrap()
        };
        assert_eq!(tags_of("Thunderbird"), ["$label1", "projekt_x"]);
        assert_eq!(
            tags_of("Dovecot"),
            ["$label4"],
            "a folded header, commas; $Forwarded isn't a tag"
        );
        assert!(tags_of("Bez").is_empty());

        // Out again: one X-Keywords with the tags, the old headers left out.
        let mut out = Vec::new();
        s.export_mbox("local/archive", &mut out).unwrap();
        let text = String::from_utf8(out.clone()).unwrap();
        assert!(
            text.contains("Status: RO\nX-Keywords: $label1 projekt_x\nSubject: Thunderbird"),
            "{text}"
        );
        assert!(!text.contains("X-Mozilla-Keys") && !text.contains("NonJunk"));
        assert!(!text.contains("$Forwarded") && !text.contains("\n $label4"));
        assert_eq!(text.matches("X-Keywords:").count(), 2, "none for no tags");
        drop(s);

        // And back in, elsewhere: the same tags.
        let again = temp("Archive.mbox");
        std::fs::write(&again, &out).unwrap();
        let key = store.lock().unwrap().create_local_folder("Kopia").unwrap();
        import_files(&store, &key, std::slice::from_ref(&again), &|_| {}).unwrap();
        let s = store.lock().unwrap();
        let copy = s.list(&key).unwrap();
        let dovecot = copy.iter().find(|m| m.subject == "Dovecot").unwrap();
        assert_eq!(s.keywords_of(dovecot.id).unwrap(), ["$label4"]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let _ = std::fs::remove_dir_all(again.parent().unwrap());
    }

    #[test]
    fn an_emlx_whose_length_is_absurd_is_read_as_mail() {
        let path = temp("huge.emlx");
        std::fs::write(&path, "18446744073709551615\nSubject: x\n\nbody\n").unwrap();
        let mut n = 0;
        read_path(&path, &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(n, 1);
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
