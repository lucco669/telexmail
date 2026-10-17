//! Outlook: its IMAP and POP3 accounts from the registry (Windows), its data files (.pst) and
//! single messages saved or dragged out of it (.msg).
//!
//! A .pst is read with Microsoft's own reader (the `outlook-pst` crate, from the MS-PST
//! specification), Unicode and old ANSI files alike. An .ost is Outlook's copy of mail kept on
//! a server: Telex downloads that mail from the server instead. A .msg is a compound file
//! (`cfb`) holding the same properties as streams.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use outlook_pst::ltp::prop_context::PropertyValue;
use outlook_pst::ltp::table_context::TableContext;
use outlook_pst::messaging::attachment::{
    AnsiAttachment, Attachment, AttachmentData, UnicodeAttachment,
};
use outlook_pst::messaging::folder::Folder;
use outlook_pst::messaging::message::{AnsiMessage, Message, UnicodeMessage};
use outlook_pst::messaging::named_prop::{NamedPropertyGuid, NamedPropertyId};
use outlook_pst::messaging::store::{AnsiStore, Store, UnicodeStore};
use outlook_pst::ndb::node_id::NodeId;
use outlook_pst::{AnsiPstFile, UnicodePstFile};

use super::mapi::{self, MapiAttachment, MapiMessage, Props, Value};
use super::{found_folder, BookPlace, FoundBook, FoundFolder, Place, Program, Source};
use crate::mailbox::FileMessage;
use crate::model::Account;
use crate::store::StoreError;
use crate::tr;
use crate::vcard::Card;

const CONTENT_COUNT: u16 = 0x3602;
const CONTAINER_CLASS: u16 = 0x3613;

/// Outlook's profiles (their IMAP and POP3 accounts) and the data files it uses or keeps in
/// its usual folders.
pub fn find(accounts: &[Account]) -> Vec<Source> {
    let mut found = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    for (profile, mut profile_accounts, data_files) in registry::profiles() {
        super::mark_present(&mut profile_accounts, accounts);
        files.extend(data_files);
        if !profile_accounts.is_empty() {
            found.push(Source {
                path: format!("outlook:{profile}"),
                program: Program::Outlook,
                name: profile,
                accounts: profile_accounts,
                folders: Vec::new(),
                filters: 0,
                tags: Vec::new(),
                filter_files: Vec::new(),
                books: Vec::new(),
            });
        }
    }
    #[cfg(not(windows))]
    let _ = accounts;
    for dir in data_dirs() {
        let pst = std::fs::read_dir(dir).into_iter().flatten().flatten();
        files.extend(pst.map(|e| e.path()));
    }
    files.retain(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("pst")));
    files.sort();
    files.dedup();
    found.extend(files.iter().filter_map(|f| read_pst(f).ok()));
    found
}

/// Where Outlook puts data files: Documents\Outlook Files, and its folder in AppData.
fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        dirs.push(home.join("Documents").join("Outlook Files"));
        dirs.push(home.join("Documents").join("Pliki programu Outlook"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(local.join("Microsoft").join("Outlook"));
    }
    dirs
}

/// A data file opened with the reader its format needs.
enum Pst {
    Unicode(Rc<UnicodeStore>),
    Ansi(Rc<AnsiStore>),
}

impl Pst {
    fn open(path: &Path) -> std::io::Result<Self> {
        match UnicodePstFile::open(path) {
            Ok(file) => Ok(Self::Unicode(UnicodeStore::read(Rc::new(file))?)),
            Err(_) => Ok(Self::Ansi(AnsiStore::read(Rc::new(AnsiPstFile::open(
                path,
            )?))?)),
        }
    }

    fn store(&self) -> Rc<dyn Store> {
        match self {
            Self::Unicode(s) => s.clone(),
            Self::Ansi(s) => s.clone(),
        }
    }

    fn folder(&self, node: NodeId) -> std::io::Result<Rc<dyn Folder>> {
        let store = self.store();
        let entry = store.properties().make_entry_id(node)?;
        store.open_folder(&entry)
    }

    /// A message with its recipients and attachments.
    fn message(&self, node: NodeId) -> std::io::Result<MapiMessage> {
        let store = self.store();
        let entry = store.properties().make_entry_id(node)?;
        match self {
            Self::Unicode(s) => {
                let m = UnicodeMessage::read(s.clone(), &entry, None)?;
                let attachments = attachment_rows(m.as_ref())
                    .into_iter()
                    .filter_map(|id| UnicodeAttachment::read(m.clone(), id, None).ok())
                    .map(|a| attachment(a.as_ref()))
                    .collect();
                Ok(mapi_message(m.as_ref(), attachments))
            }
            Self::Ansi(s) => {
                let m = AnsiMessage::read(s.clone(), &entry, None)?;
                let attachments = attachment_rows(m.as_ref())
                    .into_iter()
                    .filter_map(|id| AnsiAttachment::read(m.clone(), id, None).ok())
                    .map(|a| attachment(a.as_ref()))
                    .collect();
                Ok(mapi_message(m.as_ref(), attachments))
            }
        }
    }
}

fn unreadable_pst(path: &Path, e: &std::io::Error) -> StoreError {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) {
        StoreError::Invalid(tr!(
            "{name} is open in Outlook. Close Outlook and try again.",
            "Plik {name} jest otwarty w Outlooku. Zamknij Outlooka i spróbuj ponownie."
        ))
    } else {
        StoreError::Invalid(tr!(
            "{name} isn't an Outlook data file Telex can read ({e}).",
            "{name} nie jest plikiem danych Outlooka, który Telex umie odczytać ({e})."
        ))
    }
}

/// A data file's mail folders, with how many messages each holds.
pub fn read_pst(path: &Path) -> Result<Source, StoreError> {
    let pst = Pst::open(path).map_err(|e| unreadable_pst(path, &e))?;
    let store = pst.store();
    let root = store
        .properties()
        .ipm_sub_tree_entry_id()
        .and_then(|id| store.open_folder(&id))
        .map_err(|e| unreadable_pst(path, &e))?;
    let stem = path.file_stem().map_or_else(
        || "Outlook".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let mut folders = Vec::new();
    let mut books = Vec::new();
    walk(
        &pst,
        root.as_ref(),
        std::slice::from_ref(&stem),
        0,
        path,
        &mut folders,
        &mut books,
    );
    let bytes: u64 = std::fs::metadata(path).map_or(0, |m| m.len());
    let total: u64 = folders
        .iter()
        .filter_map(|f| f.messages)
        .sum::<u64>()
        .max(1);
    for f in &mut folders {
        // Sizes for the progress bar: the file's, shared by message count.
        f.bytes = bytes * f.messages.unwrap_or(0) / total;
    }
    Ok(Source {
        path: path.display().to_string(),
        program: Program::Outlook,
        name: path
            .file_name()
            .map_or(stem, |n| n.to_string_lossy().into_owned()),
        accounts: Vec::new(),
        folders,
        filters: 0,
        tags: Vec::new(),
        filter_files: Vec::new(),
        books,
    }
    .numbered())
}

fn text_of(value: Option<&PropertyValue>) -> Option<String> {
    match value? {
        PropertyValue::Unicode(v) => Some(String::from_utf16_lossy(v.buffer())),
        PropertyValue::String8(v) => Some(String::from_utf8_lossy(v.buffer()).into_owned()),
        _ => None,
    }
    .map(|s| s.trim_end_matches('\0').trim().to_owned())
}

fn walk(
    pst: &Pst,
    folder: &dyn Folder,
    parents: &[String],
    depth: usize,
    file: &Path,
    out: &mut Vec<FoundFolder>,
    books: &mut Vec<FoundBook>,
) {
    if depth > 16 {
        return;
    }
    let Some(table) = folder.hierarchy_table() else {
        return;
    };
    for row in table.rows_matrix() {
        let node = NodeId::from(u32::from(row.id()));
        let Ok(child) = pst.folder(node) else {
            continue;
        };
        let props = child.properties();
        let class = text_of(props.get(CONTAINER_CLASS)).unwrap_or_default();
        let contacts = class.starts_with("IPF.Contact");
        if !(contacts
            || class.is_empty()
            || class.starts_with("IPF.Note")
            || class.starts_with("IPF.Imap"))
        {
            continue; // calendars, tasks, notes, journals
        }
        let name = props.display_name().unwrap_or_default();
        let mut path = parents.to_vec();
        path.push(name.trim().to_owned());
        let count = match props.get(CONTENT_COUNT) {
            Some(PropertyValue::Integer32(n)) => u64::try_from(*n).unwrap_or(0),
            _ => 0,
        };
        if contacts {
            if count > 0 {
                books.push(FoundBook {
                    key: String::new(),
                    name: path.join(" / "),
                    contacts: count,
                    tick: true,
                    place: BookPlace::Pst {
                        file: file.to_owned(),
                        node: u32::from(node),
                    },
                });
            }
        } else if count > 0 {
            let mut f = found_folder(
                &path.join(" / "),
                Place::Pst {
                    file: file.to_owned(),
                    node: u32::from(node),
                },
                count,
            );
            f.messages = Some(count);
            out.push(f);
        }
        walk(pst, child.as_ref(), &path, depth + 1, file, out, books);
    }
}

/// The property ids this data file gives a contact's three addresses (Email1EmailAddress and
/// the next two of PSETID_Address): named properties, numbered per file.
fn address_props(pst: &Pst) -> [Option<u16>; 3] {
    let mut ids = [None; 3];
    let Ok(map) = pst.store().named_property_map() else {
        return ids;
    };
    let props = map.properties();
    let Ok(entries) = props.stream_entry() else {
        return ids;
    };
    for entry in entries {
        let NamedPropertyId::Number(lid) = entry.id() else {
            continue;
        };
        let Some(i) = EMAIL_LIDS.iter().position(|l| *l == lid) else {
            continue;
        };
        let address_set = matches!(entry.guid(), NamedPropertyGuid::GuidIndex(_))
            && props.lookup_guid(entry.guid()).is_ok_and(|g| {
                (g.data1(), g.data2(), g.data3(), *g.data4())
                    == (0x0006_2004, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46])
            });
        if address_set {
            ids[i] = Some(entry.prop_id());
        }
    }
    ids
}

/// PidLidEmail1EmailAddress, 2 and 3.
const EMAIL_LIDS: [u32; 3] = [0x8083, 0x8093, 0x80A3];
const GIVEN_NAME: u16 = 0x3A06;
const BUSINESS_PHONE: u16 = 0x3A08;
const HOME_PHONE: u16 = 0x3A09;
const SURNAME: u16 = 0x3A11;
const COMPANY: u16 = 0x3A16;
const MOBILE_PHONE: u16 = 0x3A1C;

/// A contact item as a card; `None` for anything else (distribution lists aren't read: their
/// members are a binary list of entry ids).
pub(crate) fn contact_card(props: &Props, address_ids: &[Option<u16>; 3]) -> Option<Card> {
    let class = props.text(mapi::MESSAGE_CLASS).unwrap_or_default();
    if !class.to_ascii_lowercase().starts_with("ipm.contact") {
        return None;
    }
    let name = props.text(mapi::DISPLAY_NAME).unwrap_or_else(|| {
        [GIVEN_NAME, SURNAME]
            .iter()
            .filter_map(|id| props.text(*id))
            .collect::<Vec<_>>()
            .join(" ")
    });
    Some(Card {
        name,
        // An Exchange contact's address can be an X.500 name: only SMTP addresses come.
        emails: address_ids
            .iter()
            .flatten()
            .filter_map(|id| props.text(*id))
            .filter(|e| e.contains('@'))
            .collect(),
        phone: [MOBILE_PHONE, BUSINESS_PHONE, HOME_PHONE]
            .iter()
            .filter_map(|id| props.text(*id))
            .collect::<Vec<_>>()
            .join(", "),
        organization: props.text(COMPANY).unwrap_or_default(),
        note: props.text(mapi::BODY).unwrap_or_default(),
        ..Card::default()
    })
}

/// The contacts of one contacts folder of a data file.
pub fn read_pst_contacts(path: &Path, node: u32) -> Result<Vec<Card>, StoreError> {
    let pst = Pst::open(path).map_err(|e| unreadable_pst(path, &e))?;
    let folder = pst
        .folder(NodeId::from(node))
        .map_err(|e| unreadable_pst(path, &e))?;
    let address_ids = address_props(&pst);
    let Some(contents) = folder.contents_table() else {
        return Ok(Vec::new());
    };
    Ok(contents
        .rows_matrix()
        .filter_map(|row| pst.message(NodeId::from(u32::from(row.id()))).ok())
        .filter_map(|m| contact_card(&m.props, &address_ids))
        .collect())
}

/// Read every mail message of one folder of a data file.
pub fn read_pst_folder(
    path: &Path,
    node: u32,
    each: &mut dyn FnMut(FileMessage) -> Result<(), StoreError>,
) -> Result<usize, StoreError> {
    let pst = Pst::open(path).map_err(|e| unreadable_pst(path, &e))?;
    let folder = pst
        .folder(NodeId::from(node))
        .map_err(|e| unreadable_pst(path, &e))?;
    let Some(contents) = folder.contents_table() else {
        return Ok(0);
    };
    let mut count = 0;
    for row in contents.rows_matrix() {
        let Ok(message) = pst.message(NodeId::from(u32::from(row.id()))) else {
            continue; // one damaged message doesn't stop the rest
        };
        if !mapi::is_mail(message.props.text(mapi::MESSAGE_CLASS).as_deref()) {
            continue;
        }
        each(mapi::file_message(&message))?;
        count += 1;
    }
    Ok(count)
}

fn value(v: &PropertyValue) -> Option<Value> {
    Some(match v {
        PropertyValue::Integer16(n) => Value::Int(i64::from(*n)),
        PropertyValue::Integer32(n) => Value::Int(i64::from(*n)),
        PropertyValue::Integer64(n) => Value::Int(*n),
        PropertyValue::Boolean(b) => Value::Bool(*b),
        PropertyValue::Time(t) => Value::Time(*t),
        PropertyValue::Unicode(s) => Value::Text(String::from_utf16_lossy(s.buffer())),
        PropertyValue::String8(s) => Value::Ansi(s.buffer().to_vec()),
        PropertyValue::Binary(b) => Value::Bytes(b.buffer().to_vec()),
        _ => return None,
    })
}

fn props_of<'a>(values: impl Iterator<Item = (&'a u16, &'a PropertyValue)>) -> Props {
    let mut props = Props::default();
    for (id, v) in values {
        if let Some(v) = value(v) {
            props.insert(*id, v);
        }
    }
    props.code_page = [mapi::MESSAGE_CODEPAGE, mapi::INTERNET_CPID]
        .iter()
        .find_map(|id| props.int(*id))
        .and_then(|n| u32::try_from(n).ok());
    props
}

/// The rows of a table as property bags: recipients, or attachment ids.
fn table_rows(table: &dyn TableContext) -> Vec<(u32, Props)> {
    let context = table.context();
    table
        .rows_matrix()
        .map(|row| {
            let mut props = Props::default();
            if let Ok(columns) = row.columns(context) {
                for (column, cell) in context.columns().iter().zip(columns) {
                    let Some(cell) = cell else { continue };
                    if let Some(v) = table
                        .read_column(&cell, column.prop_type())
                        .ok()
                        .as_ref()
                        .and_then(value)
                    {
                        props.insert(column.prop_id(), v);
                    }
                }
            }
            (u32::from(row.id()), props)
        })
        .collect()
}

fn attachment_rows(m: &dyn Message) -> Vec<NodeId> {
    m.attachment_table()
        .map(|t| {
            table_rows(t.as_ref())
                .into_iter()
                .map(|(id, _)| NodeId::from(id))
                .collect()
        })
        .unwrap_or_default()
}

fn mapi_message(m: &dyn Message, attachments: Vec<MapiAttachment>) -> MapiMessage {
    let props = props_of(m.properties().iter());
    let recipients = m
        .recipient_table()
        .map(|t| {
            table_rows(t.as_ref())
                .into_iter()
                .map(|(_, mut r)| {
                    r.code_page = props.code_page;
                    r
                })
                .collect()
        })
        .unwrap_or_default();
    MapiMessage {
        props,
        recipients,
        attachments,
    }
}

fn attachment(a: &dyn Attachment) -> MapiAttachment {
    let props = props_of(a.properties().iter());
    match a.data() {
        Some(AttachmentData::Binary(b)) => MapiAttachment {
            props,
            data: b.buffer().to_vec(),
            embedded: None,
        },
        Some(AttachmentData::Message(m)) => MapiAttachment {
            props,
            data: Vec::new(),
            embedded: Some(Box::new(mapi_message(m.as_ref(), Vec::new()))),
        },
        None => MapiAttachment {
            props,
            ..MapiAttachment::default()
        },
    }
}

// ---- .msg ----

/// A message saved from Outlook (.msg): its properties are streams in a compound file.
pub fn read_msg(path: &Path) -> Result<FileMessage, StoreError> {
    let unreadable = |e: std::io::Error| {
        StoreError::Invalid(tr!("can't read it: {e}", "nie można go odczytać: {e}"))
    };
    let mut file = cfb::open(path).map_err(unreadable)?;
    let message = msg_storage(&mut file, "/", 32, 0).map_err(unreadable)?;
    Ok(mapi::file_message(&message))
}

fn stream(file: &mut cfb::CompoundFile<std::fs::File>, path: &str) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    file.open_stream(path)?.read_to_end(&mut data)?;
    Ok(data)
}

/// The properties of a storage: fixed values in "__properties_version1.0" after a header
/// (32 bytes for the message, 24 for an attached one, 8 for recipients and attachments), the
/// rest in streams of their own named by id and type.
fn msg_props(
    file: &mut cfb::CompoundFile<std::fs::File>,
    dir: &str,
    header: usize,
) -> std::io::Result<Props> {
    let table = stream(file, &format!("{dir}__properties_version1.0"))?;
    let mut props = Props::default();
    for entry in table.get(header..).unwrap_or_default().as_chunks::<16>().0 {
        let tag = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]);
        let (id, kind) = ((tag >> 16) as u16, (tag & 0xFFFF) as u16);
        let fixed = &entry[8..16];
        let int = i64::from_le_bytes(fixed.try_into().unwrap_or_default());
        let v = match kind {
            0x0002 => Value::Int(i64::from(i16::from_le_bytes([fixed[0], fixed[1]]))),
            0x0003 => Value::Int(i64::from(i32::from_le_bytes([
                fixed[0], fixed[1], fixed[2], fixed[3],
            ]))),
            0x0014 => Value::Int(int),
            0x000B => Value::Bool(fixed[0] != 0),
            0x0040 => Value::Time(int),
            0x001E | 0x001F | 0x0102 => {
                let Ok(data) = stream(file, &format!("{dir}__substg1.0_{id:04X}{kind:04X}")) else {
                    continue;
                };
                match kind {
                    0x001F => {
                        let units: Vec<u16> = data
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|&c| u16::from_le_bytes(c))
                            .collect();
                        Value::Text(String::from_utf16_lossy(&units))
                    }
                    0x001E => Value::Ansi(data),
                    _ => Value::Bytes(data),
                }
            }
            _ => continue,
        };
        props.insert(id, v);
    }
    props.code_page = [mapi::MESSAGE_CODEPAGE, mapi::INTERNET_CPID]
        .iter()
        .find_map(|id| props.int(*id))
        .and_then(|n| u32::try_from(n).ok());
    Ok(props)
}

fn msg_storage(
    file: &mut cfb::CompoundFile<std::fs::File>,
    dir: &str,
    header: usize,
    depth: usize,
) -> std::io::Result<MapiMessage> {
    let props = msg_props(file, dir, header)?;
    let children: Vec<String> = file
        .read_storage(dir)?
        .map(|e| e.name().to_owned())
        .collect();
    let mut recipients = Vec::new();
    let mut attachments = Vec::new();
    for name in children {
        let sub = format!("{dir}{name}/");
        if name.starts_with("__recip_version1.0_") {
            let mut r = msg_props(file, &sub, 8)?;
            r.code_page = props.code_page;
            recipients.push(r);
        } else if name.starts_with("__attach_version1.0_") {
            let attach_props = msg_props(file, &sub, 8)?;
            let embedded_dir = format!("{sub}__substg1.0_3701000D/");
            let embedded = if depth < 4 && file.is_storage(&embedded_dir) {
                msg_storage(file, &embedded_dir, 24, depth + 1)
                    .ok()
                    .map(Box::new)
            } else {
                None
            };
            let data = attach_props
                .bytes(mapi::ATTACH_DATA)
                .map(<[u8]>::to_vec)
                .unwrap_or_default();
            attachments.push(MapiAttachment {
                props: attach_props,
                data,
                embedded,
            });
        }
    }
    Ok(MapiMessage {
        props,
        recipients,
        attachments,
    })
}

// ---- The registry (Windows) ----

#[cfg(windows)]
mod registry {
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_READ, REG_BINARY, REG_DWORD, REG_SZ,
    };

    use super::super::{identity, Blocked, FoundAccount};
    use crate::model::{AccountSetup, SmtpSecurity};

    /// The key that holds a profile's mail accounts, one subkey each.
    const ACCOUNTS: &str = "9375CFF0413111d3B88A00104B2A6676";
    /// A profile section's data file path (PR_PST_PATH_W).
    const DATA_FILE: &str = "001f6700";

    struct Key(HKEY);

    impl Key {
        fn open(parent: HKEY, path: &str) -> Option<Self> {
            let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
            let mut key: HKEY = std::ptr::null_mut();
            // SAFETY: `wide` is NUL-terminated and outlives the call; `key` receives a handle
            // that Drop closes.
            let status = unsafe { RegOpenKeyExW(parent, wide.as_ptr(), 0, KEY_READ, &mut key) };
            (status == ERROR_SUCCESS).then_some(Self(key))
        }

        fn sub(&self, path: &str) -> Option<Self> {
            Self::open(self.0, path)
        }

        fn subkeys(&self) -> Vec<String> {
            let mut names = Vec::new();
            for index in 0.. {
                let mut buf = [0u16; 256];
                let mut len = buf.len() as u32;
                // SAFETY: `buf` holds `len` units; the other out-pointers may be null.
                let status = unsafe {
                    RegEnumKeyExW(
                        self.0,
                        index,
                        buf.as_mut_ptr(),
                        &mut len,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if status != ERROR_SUCCESS {
                    break;
                }
                names.push(String::from_utf16_lossy(&buf[..len as usize]));
            }
            names
        }

        /// A value as text (REG_SZ, or UTF-16 in REG_BINARY as Outlook writes it) or a number.
        fn value(&self, name: &str) -> Option<RegValue> {
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let mut kind = 0u32;
            let mut size = 0u32;
            // SAFETY: a null data pointer asks only for the type and size.
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    wide.as_ptr(),
                    std::ptr::null(),
                    &mut kind,
                    std::ptr::null_mut(),
                    &mut size,
                )
            };
            if status != ERROR_SUCCESS || size > 64 * 1024 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            // SAFETY: `data` holds `size` bytes.
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    wide.as_ptr(),
                    std::ptr::null(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut size,
                )
            };
            if status != ERROR_SUCCESS {
                return None;
            }
            data.truncate(size as usize);
            match kind {
                REG_DWORD if data.len() >= 4 => Some(RegValue::Number(u32::from_le_bytes([
                    data[0], data[1], data[2], data[3],
                ]))),
                REG_SZ | REG_BINARY => {
                    let units: Vec<u16> = data
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&c| u16::from_le_bytes(c))
                        .collect();
                    let text = String::from_utf16_lossy(&units);
                    Some(RegValue::Text(
                        text.trim_end_matches('\0').trim().to_owned(),
                    ))
                }
                _ => None,
            }
        }

        fn text(&self, name: &str) -> Option<String> {
            match self.value(name)? {
                RegValue::Text(t) if !t.is_empty() => Some(t),
                _ => None,
            }
        }

        fn number(&self, name: &str) -> Option<u32> {
            match self.value(name)? {
                RegValue::Number(n) => Some(n),
                RegValue::Text(_) => None,
            }
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    enum RegValue {
        Text(String),
        Number(u32),
    }

    /// Each Outlook profile: its name, its accounts, and the .pst files it opens.
    pub fn profiles() -> Vec<(String, Vec<FoundAccount>, Vec<PathBuf>)> {
        let mut out: Vec<(String, Vec<FoundAccount>, Vec<PathBuf>)> = Vec::new();
        for version in ["16.0", "15.0"] {
            let path = format!(r"Software\Microsoft\Office\{version}\Outlook\Profiles");
            let Some(profiles) = Key::open(HKEY_CURRENT_USER, &path) else {
                continue;
            };
            for name in profiles.subkeys() {
                if out.iter().any(|(n, _, _)| n == &name) {
                    continue;
                }
                let Some(profile) = profiles.sub(&name) else {
                    continue;
                };
                let mut accounts = Vec::new();
                if let Some(list) = profile.sub(ACCOUNTS) {
                    for id in list.subkeys() {
                        if let Some(a) = list.sub(&id).and_then(|k| account(&k, &id)) {
                            accounts.push(a);
                        }
                    }
                }
                let files = profile
                    .subkeys()
                    .iter()
                    .filter_map(|s| profile.sub(s)?.text(DATA_FILE))
                    .map(PathBuf::from)
                    .collect();
                out.push((name, accounts, files));
            }
        }
        out
    }

    /// "SMTP Secure Connection" and the like: 1 is SSL/TLS, 2 STARTTLS, 3 automatic.
    fn security(key: &Key, prefix: &str) -> SmtpSecurity {
        let secure = key.number(&format!("{prefix} Secure Connection"));
        let ssl = key.number(&format!("{prefix} Use SSL"));
        match (secure, ssl) {
            (Some(2), _) => SmtpSecurity::StartTls,
            (Some(1), _) | (_, Some(1)) => SmtpSecurity::Tls,
            _ => SmtpSecurity::StartTls,
        }
    }

    fn account(key: &Key, id: &str) -> Option<FoundAccount> {
        let email = key.text("Email").or_else(|| key.text("Account Name"))?;
        if !email.contains('@') {
            return None;
        }
        let name = key
            .text("Display Name")
            .filter(|n| n != &email)
            .unwrap_or_else(|| email.clone());
        let (host, user, prefix, blocked) = if let Some(host) = key.text("IMAP Server") {
            (host, key.text("IMAP User"), "IMAP", None)
        } else if let Some(host) = key.text("POP3 Server") {
            (host, key.text("POP3 User"), "POP3", Some(Blocked::Pop3))
        } else {
            (String::new(), None, "", Some(Blocked::Exchange))
        };
        let port = |name: &str| {
            key.number(&format!("{name} Port"))
                .and_then(|p| u16::try_from(p).ok())
                .unwrap_or(0)
        };
        Some(FoundAccount {
            key: id.to_owned(),
            setup: AccountSetup {
                name: name.clone(),
                email: email.clone(),
                host,
                port: if prefix == "IMAP" { port("IMAP") } else { 0 },
                username: user.unwrap_or_default(),
                imap_security: security(key, prefix),
                smtp_host: key.text("SMTP Server").unwrap_or_default(),
                smtp_port: port("SMTP"),
                smtp_security: security(key, "SMTP"),
                pop3: None,
            },
            identities: vec![identity(&name, &email, "")],
            present: false,
            blocked,
            seen: Vec::new(),
            seen_in: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::tests::temp_dir;
    use pst_fixture::{
        UnicodePstAttachment, UnicodePstFile as FixtureFile, UnicodePstMessage,
        UnicodePstRecipient, UnicodePstRecipientType,
    };

    fn message<'a>(
        subject: &'a str,
        id: &'a str,
        recipients: &'a [UnicodePstRecipient<'a>],
    ) -> UnicodePstMessage<'a> {
        UnicodePstMessage {
            subject,
            sender_name: "Anna Kowalska",
            sender_email: "anna@firma.pl",
            recipients,
            body: "Dzień dobry,\nw załączniku wycena.",
            html_body: Some("<p>Dzień dobry, <img src=\"cid:logo@firma.pl\"></p>"),
            message_id: id,
            delivery_time: 134_042_256_000_000_000,
        }
    }

    #[test]
    fn an_outlook_data_file_comes_in_folder_by_folder() {
        let dir = temp_dir("pst");
        let path = dir.join("Archiwum.pst");
        let to = [UnicodePstRecipient {
            name: "Piotr Nowak",
            email: "piotr@nowak-meble.pl",
            recipient_type: UnicodePstRecipientType::To,
        }];
        let png = [0x89, b'P', b'N', b'G', 0, 0, 0, 0];
        drop(
            FixtureFile::create_with_attachments(
                &path,
                &message("Wycena łóżka", "<w1@firma.pl>", &to),
                &[
                    UnicodePstAttachment {
                        filename: "logo.png",
                        mime_type: "image/png",
                        content_id: Some("<logo@firma.pl>"),
                        data: &png,
                    },
                    UnicodePstAttachment {
                        filename: "wycena.pdf",
                        mime_type: "application/pdf",
                        content_id: None,
                        data: b"%PDF-1.7",
                    },
                ],
            )
            .unwrap(),
        );
        drop(
            FixtureFile::append_in_folder(
                &path,
                &["Klienci", "2026"],
                &message("Termin", "<t2@firma.pl>", &to),
            )
            .unwrap(),
        );

        let source = read_pst(&path).unwrap();
        let names: Vec<&str> = source.folders.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"Archiwum / Klienci / 2026"), "{names:?}");
        assert!(
            source.books.is_empty(),
            "an empty Contacts folder isn't offered"
        );
        let inbox = source
            .folders
            .iter()
            .find(|f| f.messages == Some(1) && f.name != "Archiwum / Klienci / 2026")
            .unwrap();
        let mut read = Vec::new();
        let n = match &inbox.place {
            Place::Pst { file, node } => read_pst_folder(file, *node, &mut |m| {
                read.push(m);
                Ok(())
            })
            .unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(n, 1);
        let parsed = crate::mime::parse_message(&read[0].raw, "local/inbox", read[0].date);
        assert_eq!(parsed.subject, "Wycena łóżka");
        assert_eq!(parsed.from_email, "anna@firma.pl");
        assert!(parsed.to.contains("piotr@nowak-meble.pl"));
        assert_eq!(parsed.message_id.as_deref(), Some("w1@firma.pl"));
        assert!(parsed.body.contains("w załączniku wycena"));
        let names: Vec<&str> = parsed.attachments.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            ["wycena.pdf"],
            "the logo shows in the text, it isn't a file"
        );
        let raw = String::from_utf8_lossy(&read[0].raw);
        assert!(
            raw.contains("Content-ID: <logo@firma.pl>"),
            "the logo goes with the HTML"
        );
        let html = crate::mime::html_body(&read[0].raw).unwrap();
        assert!(html.html.contains("cid:logo@firma.pl"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A property stream entry: tag (type, id), flags, then the value or the size of the
    /// stream that holds it.
    fn entry(id: u16, kind: u16, value: u64) -> Vec<u8> {
        let mut e = (u32::from(id) << 16 | u32::from(kind))
            .to_le_bytes()
            .to_vec();
        e.extend_from_slice(&6u32.to_le_bytes());
        e.extend_from_slice(&value.to_le_bytes());
        e
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn a_message_saved_from_outlook_comes_in() {
        use std::io::Write;
        let dir = temp_dir("msg");
        let path = dir.join("Wycena.msg");
        let mut file = cfb::create(&path).unwrap();
        let put = |file: &mut cfb::CompoundFile<std::fs::File>, name: &str, data: &[u8]| {
            file.create_stream(name).unwrap().write_all(data).unwrap();
        };
        let subject = utf16("Wycena łóżka");
        let body = utf16("Dzień dobry,\r\nw załączniku wycena.");
        let mut props = vec![0u8; 32];
        props.extend(entry(0x0037, 0x001F, subject.len() as u64));
        props.extend(entry(0x1000, 0x001F, body.len() as u64));
        props.extend(entry(0x0E07, 0x0003, 1)); // read
        props.extend(entry(0x0039, 0x0040, 134_042_256_000_000_000));
        props.extend(entry(0x0C1A, 0x001F, 26));
        props.extend(entry(0x5D01, 0x001F, 26));
        put(&mut file, "/__properties_version1.0", &props);
        put(&mut file, "/__substg1.0_0037001F", &subject);
        put(&mut file, "/__substg1.0_1000001F", &body);
        put(&mut file, "/__substg1.0_0C1A001F", &utf16("Anna Kowalska"));
        put(&mut file, "/__substg1.0_5D01001F", &utf16("anna@firma.pl"));
        file.create_storage("/__recip_version1.0_#00000000")
            .unwrap();
        let mut recip = vec![0u8; 8];
        recip.extend(entry(0x0C15, 0x0003, 1));
        recip.extend(entry(0x3001, 0x001F, 22));
        recip.extend(entry(0x39FE, 0x001F, 40));
        put(
            &mut file,
            "/__recip_version1.0_#00000000/__properties_version1.0",
            &recip,
        );
        put(
            &mut file,
            "/__recip_version1.0_#00000000/__substg1.0_3001001F",
            &utf16("Piotr Nowak"),
        );
        put(
            &mut file,
            "/__recip_version1.0_#00000000/__substg1.0_39FE001F",
            &utf16("piotr@nowak-meble.pl"),
        );
        file.create_storage("/__attach_version1.0_#00000000")
            .unwrap();
        let mut attach = vec![0u8; 8];
        attach.extend(entry(0x3707, 0x001F, 20));
        attach.extend(entry(0x3701, 0x0102, 8));
        put(
            &mut file,
            "/__attach_version1.0_#00000000/__properties_version1.0",
            &attach,
        );
        put(
            &mut file,
            "/__attach_version1.0_#00000000/__substg1.0_3707001F",
            &utf16("wycena.pdf"),
        );
        put(
            &mut file,
            "/__attach_version1.0_#00000000/__substg1.0_37010102",
            b"%PDF-1.7",
        );
        file.flush().unwrap();
        drop(file);

        let m = read_msg(&path).unwrap();
        assert!(m.seen && !m.flagged);
        let parsed = crate::mime::parse_message(&m.raw, "local/inbox", m.date);
        assert_eq!(parsed.subject, "Wycena łóżka");
        assert_eq!(parsed.from_email, "anna@firma.pl");
        assert!(parsed.to.contains("piotr@nowak-meble.pl"), "{}", parsed.to);
        assert!(parsed.body.contains("w załączniku wycena"));
        assert_eq!(parsed.attachments[0].name, "wycena.pdf");
        assert_eq!(parsed.date.format("%Y-%m-%d").to_string(), "2025-10-06");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn contact_items_become_cards() {
        let mut props = Props::default();
        let text = |s: &str| Value::Text(s.to_owned());
        props.insert(mapi::MESSAGE_CLASS, text("IPM.Contact"));
        props.insert(GIVEN_NAME, text("Małgorzata"));
        props.insert(SURNAME, text("Wróblewska"));
        props.insert(COMPANY, text("Piżama Sp. z o.o."));
        props.insert(MOBILE_PHONE, text("600 700 800"));
        props.insert(BUSINESS_PHONE, text("22 111 22 33"));
        props.insert(mapi::BODY, text("Zamówienia w piątki"));
        // This file numbers Email1 as 0x8005 and Email2 as 0x8006; Email2 is an Exchange name.
        props.insert(0x8005, text("malgosia@pizama.pl"));
        props.insert(
            0x8006,
            text("/o=ExchangeLabs/ou=Exchange/cn=Recipients/cn=mw"),
        );
        let ids = [Some(0x8005), Some(0x8006), None];
        let card = contact_card(&props, &ids).unwrap();
        assert_eq!(card.name, "Małgorzata Wróblewska");
        assert_eq!(card.emails, ["malgosia@pizama.pl"]);
        assert_eq!(card.phone, "600 700 800, 22 111 22 33");
        assert_eq!(card.organization, "Piżama Sp. z o.o.");
        assert_eq!(card.note, "Zamówienia w piątki");
        props.insert(mapi::DISPLAY_NAME, text("Gosia W."));
        assert_eq!(contact_card(&props, &ids).unwrap().name, "Gosia W.");
        props.insert(mapi::MESSAGE_CLASS, text("IPM.DistList"));
        assert!(contact_card(&props, &ids).is_none());
    }

    #[test]
    fn a_file_that_isnt_a_data_file_says_so() {
        let dir = temp_dir("notpst");
        let path = dir.join("zepsuty.pst");
        std::fs::write(&path, b"not a pst at all").unwrap();
        let e = read_pst(&path).unwrap_err().to_string();
        assert!(e.contains("zepsuty.pst"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
