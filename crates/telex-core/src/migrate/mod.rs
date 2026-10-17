//! Moving in from another mail program: Thunderbird (and Betterbird, SeaMonkey, Postbox),
//! Outlook, and the Maildir and MH folders of Evolution, KMail, Claws Mail and mutt.
//!
//! Finding reads only files on this PC (and on Windows, Outlook's entries in the registry);
//! nothing goes over the network. What a program has comes as a [`Source`]: its accounts
//! (servers, identities, signatures; never passwords, which the user types once), the mail it
//! keeps on this PC (Thunderbird's Local Folders, Outlook's .pst files, Maildir folders), and
//! its filters. Bringing it in adds the accounts ([`MailStore::add_found_account`]), reads the
//! chosen folders into folders on this PC ([`import_mail`]), and makes rules of the filters
//! that Telex can do the same way ([`rules`]). Mail on an IMAP server isn't copied from the
//! other program: Telex downloads it from the server.

pub mod mapi;
pub mod outlook;
pub mod thunderbird;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::contacts;
use crate::mailbox::{self, ImportProgress, ImportReport, Importer};
use crate::model::{Account, AccountSetup, FolderKind, Identity};
use crate::settings::{self, PlanItem};
use crate::store::{MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// The programs Telex can move in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Program {
    Thunderbird,
    Betterbird,
    Seamonkey,
    Postbox,
    Outlook,
    Evolution,
    Kmail,
    /// A folder of mail chosen by hand, or one found where mail programs keep it (~/Mail).
    Folder,
}

/// What one program (one profile of it, or one Outlook data file) has.
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    /// Where it is: a profile folder, a .pst file, a mail folder. Names it in [`open`].
    pub path: String,
    pub program: Program,
    /// The profile's or the file's name.
    pub name: String,
    pub accounts: Vec<FoundAccount>,
    pub folders: Vec<FoundFolder>,
    /// How many filters it has.
    pub filters: usize,
    #[serde(skip)]
    filter_files: Vec<(String, PathBuf)>,
    /// Its address books.
    pub books: Vec<FoundBook>,
}

/// An address book another program keeps: one of Thunderbird's, the Contacts of an Outlook
/// data file.
#[derive(Debug, Clone, Serialize)]
pub struct FoundBook {
    pub key: String,
    pub name: String,
    /// How many cards it holds (contacts and lists).
    pub contacts: u64,
    /// Ticked to begin with: not Thunderbird's collected addresses, which it gathers itself.
    pub tick: bool,
    #[serde(skip)]
    place: BookPlace,
}

#[derive(Debug, Clone)]
enum BookPlace {
    /// Thunderbird's abook.sqlite and its kin.
    Sqlite(PathBuf),
    /// A contacts folder of an Outlook data file, by its node.
    Pst { file: PathBuf, node: u32 },
}

/// An account another program has.
#[derive(Debug, Clone, Serialize)]
pub struct FoundAccount {
    /// Its name in the other program's settings ("server5", "00000002").
    pub key: String,
    pub setup: AccountSetup,
    /// Its identities, the first being the default.
    pub identities: Vec<Identity>,
    /// Already set up in Telex.
    pub present: bool,
    /// Why Telex can't use it (yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Blocked>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocked {
    /// It signs in with Google or Microsoft (OAuth2), which Telex doesn't have yet.
    Oauth,
    /// POP3: Telex reads mail over IMAP only. Its mail on this PC can still come along.
    Pop3,
    /// Exchange or Microsoft 365 in Outlook.
    Exchange,
}

/// Mail a program keeps on this PC: a folder of it.
#[derive(Debug, Clone, Serialize)]
pub struct FoundFolder {
    pub key: String,
    /// "Archives / 2019": also the name of the folder on this PC it goes into.
    pub name: String,
    pub bytes: u64,
    /// When the program says so cheaply (a .pst does).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<u64>,
    /// Ticked to begin with: not Trash, Junk, Outbox or templates.
    pub tick: bool,
    #[serde(skip)]
    place: Place,
}

#[derive(Debug, Clone)]
enum Place {
    /// An mbox file, a Maildir or MH folder.
    Path(PathBuf),
    /// A folder of an Outlook data file, by its node.
    Pst { file: PathBuf, node: u32 },
}

/// What the user ticked.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Choice {
    /// Keys of the accounts to add.
    #[serde(default)]
    pub accounts: Vec<String>,
    /// Passwords typed in, by account key.
    #[serde(default)]
    pub passwords: HashMap<String, String>,
    /// Keys of the folders to read into folders on this PC.
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub filters: bool,
    /// Keys of the address books to bring.
    #[serde(default)]
    pub books: Vec<String>,
}

/// Programs and their mail on this PC, the most used first.
pub fn find(accounts: &[Account]) -> Vec<Source> {
    let mut found: Vec<Source> = Vec::new();
    for (root, program) in thunderbird::roots() {
        for profile in thunderbird::profiles(&root) {
            if let Some(source) = thunderbird::read(&profile, program, accounts) {
                if !found.iter().any(|s| s.path == source.path) {
                    found.push(source);
                }
            }
        }
    }
    found.extend(outlook::find(accounts));
    for (dir, program) in mail_dir_roots() {
        if let Some(source) = read_mail_folder(&dir, program) {
            found.push(source);
        }
    }
    found
}

/// One source by its path, as [`find`] names it or as the user picked it: a profile folder,
/// an Outlook data file (.pst), or a folder of mail (Maildir, MH, mbox files).
pub fn open(path: &Path, accounts: &[Account]) -> Result<Source, StoreError> {
    let is_pst = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pst"));
    let found = if is_pst {
        Some(outlook::read_pst(path)?)
    } else if path.join("prefs.js").is_file() {
        thunderbird::read(path, thunderbird::program_of(path), accounts)
    } else if path.is_dir() {
        let program = mail_dir_roots()
            .into_iter()
            .find(|(dir, _)| dir == path)
            .map_or(Program::Folder, |(_, p)| p);
        read_mail_folder(path, program)
    } else {
        outlook::find(accounts)
            .into_iter()
            .find(|s| Path::new(&s.path) == path)
    };
    found.ok_or_else(|| {
        StoreError::Invalid(tr!(
            "Telex found no mail there.",
            "Telex nie znalazł tam poczty."
        ))
    })
}

/// Where Linux programs keep mail as Maildir or MH folders.
fn mail_dir_roots() -> Vec<(PathBuf, Program)> {
    if cfg!(windows) {
        return Vec::new();
    }
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    [
        (data.join("evolution/mail/local"), Program::Evolution),
        (data.join("local-mail"), Program::Kmail),
        (home.join("Mail"), Program::Folder),
        (home.join("Maildir"), Program::Folder),
    ]
    .into_iter()
    .filter(|(dir, _)| dir.is_dir())
    .collect()
}

/// The folders of mail under `root`: Maildir (also Maildir++ with ".Sent", ".Archive.2019"
/// and KMail's ".inbox.directory"), MH, and mbox files.
fn read_mail_folder(root: &Path, program: Program) -> Option<Source> {
    let mut folders = Vec::new();
    let mut budget = SCAN_BUDGET;
    collect_mail_dirs(root, &[], 0, &mut budget, &mut folders);
    if folders.is_empty() {
        return None;
    }
    let name = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    Some(
        Source {
            path: root.display().to_string(),
            program,
            name,
            accounts: Vec::new(),
            folders,
            filters: 0,
            filter_files: Vec::new(),
            books: Vec::new(),
        }
        .numbered(),
    )
}

/// How many files and folders a scan looks at, at most: a folder picked by mistake (a whole
/// disk) is given up on instead of read through.
const SCAN_BUDGET: usize = 20_000;

fn collect_mail_dirs(
    dir: &Path,
    parents: &[String],
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<FoundFolder>,
) {
    if depth > 8 || *budget == 0 {
        return;
    }
    if mailbox::is_mail_dir(dir) {
        let name = if parents.is_empty() {
            "Inbox".to_owned()
        } else {
            parents.join(" / ")
        };
        out.push(found_folder(
            &name,
            Place::Path(dir.to_owned()),
            dir_size(dir),
        ));
    }
    let mut entries: Vec<(String, PathBuf, bool)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            let is_dir = e.file_type().ok()?.is_dir();
            Some((name, e.path(), is_dir))
        })
        .collect();
    entries.sort();
    for (name, path, is_dir) in entries {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        // Maildir's own folders, and MH's message files (a folder may well be called "2019").
        if matches!(name.as_str(), "cur" | "new" | "tmp") || !is_dir && name.parse::<u32>().is_ok()
        {
            continue;
        }
        if is_dir {
            // KMail: ".inbox.directory" holds inbox's subfolders.
            if let Some(owner) = name
                .strip_prefix('.')
                .and_then(|n| n.strip_suffix(".directory"))
            {
                let mut path_names = parents.to_vec();
                path_names.push(owner.to_owned());
                collect_children(&path, &path_names, depth, budget, out);
            } else if let Some(dotted) = name.strip_prefix('.') {
                // Maildir++: ".Archive.2019" is Archive / 2019.
                if mailbox::is_mail_dir(&path) {
                    let mut path_names = parents.to_vec();
                    path_names.extend(dotted.split('.').map(str::to_owned));
                    let label = path_names.join(" / ");
                    out.push(found_folder(
                        &label,
                        Place::Path(path.clone()),
                        dir_size(&path),
                    ));
                }
            } else {
                let mut path_names = parents.to_vec();
                path_names.push(name.clone());
                collect_mail_dirs(&path, &path_names, depth + 1, budget, out);
            }
        } else if looks_like_mbox(&path) {
            let mut path_names = parents.to_vec();
            path_names.push(name.trim_end_matches(".mbox").to_owned());
            let size = std::fs::metadata(&path).map_or(0, |m| m.len());
            out.push(found_folder(
                &path_names.join(" / "),
                Place::Path(path),
                size,
            ));
        }
    }
}

fn collect_children(
    dir: &Path,
    parents: &[String],
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<FoundFolder>,
) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let (Some(name), true) = (
            entry.file_name().to_str().map(str::to_owned),
            entry.file_type().is_ok_and(|t| t.is_dir()),
        ) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let mut path_names = parents.to_vec();
        path_names.push(name);
        collect_mail_dirs(&entry.path(), &path_names, depth + 1, budget, out);
    }
}

/// A file that starts like mbox ("From ").
pub(crate) fn looks_like_mbox(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 5];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && &head == b"From "
}

pub(crate) fn dir_size(dir: &Path) -> u64 {
    ["cur", "new", ""]
        .iter()
        .flat_map(|sub| {
            std::fs::read_dir(dir.join(sub))
                .into_iter()
                .flatten()
                .flatten()
        })
        .filter_map(|e| e.metadata().ok().filter(|m| m.is_file()))
        .map(|m| m.len())
        .sum()
}

/// A folder as the user will see it ticked or not. Trash, junk, outboxes and templates start
/// unticked: what they hold was thrown away or isn't mail yet.
fn found_folder(name: &str, place: Place, bytes: u64) -> FoundFolder {
    let leaf = name
        .rsplit(" / ")
        .next()
        .unwrap_or(name)
        .trim()
        .to_lowercase();
    let unwanted = [
        "trash",
        "deleted",
        "deleted items",
        "deleted messages",
        "kosz",
        "elementy usunięte",
        "junk",
        "junk e-mail",
        "junk email",
        "spam",
        "wiadomości-śmieci",
        "outbox",
        "unsent messages",
        "skrzynka nadawcza",
        "templates",
        "szablony",
        "sync issues",
        "problemy z synchronizacją",
        "rss feeds",
        "kanały informacyjne rss",
        "conversation history",
        "historia konwersacji",
        "queue",
    ];
    FoundFolder {
        key: String::new(),
        name: name.to_owned(),
        bytes,
        messages: None,
        tick: bytes > 0 && !unwanted.contains(&leaf.as_str()),
        place,
    }
}

impl Source {
    /// Number the folders and address books: keys are what the UI sends back.
    fn numbered(mut self) -> Self {
        for (i, f) in self.folders.iter_mut().enumerate() {
            f.key = format!("f{i}");
        }
        for (i, b) in self.books.iter_mut().enumerate() {
            b.key = format!("b{i}");
        }
        self
    }
}

/// Bring the chosen address books of `source` into the address book. A contact that is there
/// already (same UID or address) is only filled in, so moving in twice brings nothing twice.
pub fn import_books(
    store: &mut MailStore,
    source: &Source,
    keys: &[String],
) -> Result<contacts::ImportReport, StoreError> {
    let mut report = contacts::ImportReport::default();
    for book in source.books.iter().filter(|b| keys.contains(&b.key)) {
        let cards = match &book.place {
            BookPlace::Sqlite(path) => thunderbird::read_book(path)?,
            BookPlace::Pst { file, node } => outlook::read_pst_contacts(file, *node)?,
        };
        report.add(&store.import_cards(&cards)?);
    }
    Ok(report)
}

impl MailStore {
    /// Add an account found in another program, with its identities instead of the usual
    /// default one. A first account makes the folders on this PC, as any first account does.
    pub fn add_found_account(
        &mut self,
        setup: &AccountSetup,
        identities: &[Identity],
    ) -> Result<Account, StoreError> {
        let account = self.upsert_account(setup)?;
        let made = account.identities.first().map(|i| i.id);
        for (i, found) in identities.iter().enumerate() {
            let identity = Identity {
                id: if i == 0 { made.unwrap_or(0) } else { 0 },
                account: account.id.clone(),
                is_default: i == 0,
                ..found.clone()
            };
            if identity.check().is_ok() {
                self.save_identity(&identity)?;
            }
        }
        self.account(&account.id).cloned()
    }

    /// The folder on this PC called `name`, made when there is none. Archive and Trash answer
    /// to their names in either language.
    pub fn local_folder_named(&mut self, name: &str) -> Result<String, StoreError> {
        // As a folder's name is kept: spaces collapsed, at most 80 characters.
        let name: String = name
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(80)
            .collect();
        let wanted = name.to_lowercase();
        if let Some(pc) = self.this_pc() {
            let same = |f: &&crate::model::Folder| {
                f.kind != FolderKind::Outbox
                    && (f.name.to_lowercase() == wanted
                        || f.kind
                            .pl_name()
                            .is_some_and(|pl| pl.to_lowercase() == wanted)
                        || (f.kind == FolderKind::Archive && wanted == "archives"))
            };
            if let Some(f) = pc.folders.iter().find(same) {
                return Ok(pc.folder_key(&f.id));
            }
        }
        self.create_local_folder(&name)
    }
}

/// Read the chosen folders of `source` into folders on this PC of the same names, skipping
/// mail that is there already, so moving in twice brings nothing twice. Progress counts
/// bytes, as the folders' sizes are known before their messages are.
pub fn import_mail(
    store: &Mutex<MailStore>,
    source: &Source,
    keys: &[String],
    progress: &(dyn Fn(ImportProgress) + Send + Sync),
) -> Result<ImportReport, StoreError> {
    let mut report = ImportReport::default();
    let (mut done, mut bytes) = (0, 0);
    for folder in source.folders.iter().filter(|f| keys.contains(&f.key)) {
        let key = store
            .lock()
            .map_err(|_| StoreError::poisoned())?
            .local_folder_named(&folder.name)?;
        let (base_done, base_bytes) = (done, bytes);
        let shifted = |p: ImportProgress| {
            progress(ImportProgress {
                done: base_done + p.done,
                bytes: base_bytes + p.bytes,
                file: p.file,
            });
        };
        let mut importer = Importer::new(store, &key, &shifted)?;
        match &folder.place {
            Place::Path(path) => importer.read(path, &folder.name)?,
            Place::Pst { file, node } => {
                let read = outlook::read_pst_folder(file, *node, &mut |m| importer.add(m));
                importer.flush()?;
                if let Err(e) = read {
                    importer
                        .report
                        .problems
                        .push(format!("{}: {e}", folder.name));
                }
            }
        }
        done += importer.done;
        bytes += importer.bytes.max(folder.bytes);
        report.add(importer.report);
    }
    Ok(report)
}

/// Make rules of the source's filters, where Telex can do what they do, and save them.
/// Returns what happened to each, as a settings import shows it.
pub fn rules(source: &Source, store: &mut MailStore) -> Vec<PlanItem> {
    let mut entries = Vec::new();
    let mut unsupported = Vec::new();
    for (key, file) in &source.filter_files {
        let Some(account) = source.accounts.iter().find(|a| &a.key == key) else {
            continue;
        };
        for filter in thunderbird::filters(file, account, source) {
            match filter {
                Ok(entry) => entries.push(entry),
                Err(item) => unsupported.push(item),
            }
        }
    }
    let (mut items, rules) = settings::plan_rules(&entries, store);
    for (_, rule) in rules {
        if let Err(e) = store.save_rule(&rule) {
            if let Some(item) = items.iter_mut().find(|i| i.name == rule.name) {
                item.status = settings::Status::Skip;
                item.skip = Some(settings::Skip::Invalid {
                    detail: e.to_string(),
                });
            }
        }
    }
    items.extend(unsupported);
    items
}

/// The account's identities as Telex keeps them: name, address and signature, with Telex's
/// own reply and forward headers.
fn identity(name: &str, email: &str, signature: &str) -> Identity {
    Identity {
        signature: signature.trim_end().to_owned(),
        ..Identity::new("", name.trim(), email.trim())
    }
}

/// Marks the accounts already in Telex.
fn mark_present(found: &mut [FoundAccount], accounts: &[Account]) {
    for f in found {
        f.present = accounts
            .iter()
            .any(|a| a.id != LOCAL_ID && a.email.eq_ignore_ascii_case(&f.setup.email));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "telex-migrate-{name}-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn maildir_and_mh_folders_are_found_and_read() {
        let root = temp_dir("maildir");
        // Maildir++ as Evolution keeps it: the Inbox at the top, ".Archive.2019" below it.
        for sub in [
            "cur",
            "new",
            "tmp",
            ".Archive.2019/cur",
            ".Archive.2019/new",
        ] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        let mail = |subject: &str| {
            format!("From: a@x.pl\nSubject: {subject}\nMessage-ID: <{subject}@x.pl>\n\nTreść\n")
        };
        // Windows can't have ":" in a name: Maildir copies there use "!".
        let info = if cfg!(windows) { "!2," } else { ":2," };
        std::fs::write(root.join(format!("cur/1.host{info}S")), mail("przeczytana")).unwrap();
        std::fs::write(root.join(format!("cur/2.host{info}FS")), mail("flaga")).unwrap();
        std::fs::write(root.join(format!("cur/3.host{info}ST")), mail("usunieta")).unwrap();
        std::fs::write(root.join("new/4.host"), mail("nowa")).unwrap();
        std::fs::write(
            root.join(format!(".Archive.2019/cur/5.host{info}S")),
            mail("stara"),
        )
        .unwrap();
        // MH as Claws Mail keeps it: numbered files and the unseen sequence.
        let mh = root.join("claws");
        std::fs::create_dir_all(&mh).unwrap();
        std::fs::write(mh.join("1"), mail("mh-jeden")).unwrap();
        std::fs::write(mh.join("2"), mail("mh-dwa")).unwrap();
        std::fs::write(mh.join(".mh_sequences"), "unseen: 2\n").unwrap();

        // A folder named by a number is a folder; MH's numbered files are messages.
        std::fs::create_dir_all(root.join("2019").join("cur")).unwrap();
        std::fs::write(root.join("2019").join("cur").join("9.host"), mail("rok")).unwrap();
        let source = read_mail_folder(&root, Program::Evolution).unwrap();
        let names: Vec<&str> = source.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Inbox", "Archive / 2019", "2019", "claws"]);
        let keys: Vec<&str> = source.folders.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, ["f0", "f1", "f2", "f3"], "each its own key");

        let store = Mutex::new(MailStore::in_memory().unwrap());
        store
            .lock()
            .unwrap()
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        let keys: Vec<String> = source.folders.iter().map(|f| f.key.clone()).collect();
        let report = import_mail(&store, &source, &keys, &|_| {}).unwrap();
        assert_eq!(report.added, 7, "{report:?}");
        let s = store.lock().unwrap();
        let inbox = s.list("local/inbox").unwrap();
        let subject = |name: &str| inbox.iter().find(|m| m.subject == name).unwrap().clone();
        assert!(!subject("przeczytana").unread);
        assert!(subject("flaga").flagged);
        assert!(subject("nowa").unread, "mail in new/ is unread");
        assert!(inbox.iter().all(|m| m.subject != "usunieta"), "T: deleted");
        assert_eq!(s.list("local/archive-2019").unwrap().len(), 1);
        let claws = s.list("local/claws").unwrap();
        assert!(
            !claws
                .iter()
                .find(|m| m.subject == "mh-jeden")
                .unwrap()
                .unread
        );
        assert!(claws.iter().find(|m| m.subject == "mh-dwa").unwrap().unread);
        drop(s);
        // Again: nothing twice.
        let again = import_mail(&store, &source, &keys, &|_| {}).unwrap();
        assert_eq!((again.added, again.already), (0, 7));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn archive_and_trash_answer_to_their_names() {
        let mut store = MailStore::in_memory().unwrap();
        store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            store.local_folder_named("Archives").unwrap(),
            "local/archive"
        );
        assert_eq!(store.local_folder_named("Kosz").unwrap(), "local/trash");
        let made = store.local_folder_named("Faktury / 2026").unwrap();
        assert_eq!(store.local_folder_named("faktury / 2026").unwrap(), made);
        assert_eq!(
            store.local_folder_named("Faktury  /  2026 ").unwrap(),
            made,
            "spaces as kept"
        );
    }
}
