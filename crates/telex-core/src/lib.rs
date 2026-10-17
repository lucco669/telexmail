//! Telex mail engine.
//!
//! Everything that is not UI lives here, so it can be tested with plain
//! `cargo test -p telex-core` and without a window. The Tauri app in
//! `src-tauri` is a thin layer that exposes these functions to the UI.
//!
//! `MailStore` is the local mail base in SQLite. `imap` fills it from a server,
//! read-only for now; `mime` turns raw messages into `Message`s.

pub mod attachments;
pub mod autoconfig;
pub mod away;
pub mod backup;
pub mod calendar;
mod colors;
pub mod contacts;
pub mod count;
pub mod drafts;
pub mod export;
pub mod filters;
pub mod html;
pub mod i18n;
pub mod images;
pub mod imap;
pub mod later;
pub mod list;
pub mod mailbox;
pub mod markdown;
pub mod migrate;
pub mod mime;
pub mod model;
pub mod pop3;
pub mod openpgp;
pub mod recipients;
pub mod risk;
#[cfg(test)]
pub(crate) mod sample;
pub mod saved_searches;
pub mod servers;
pub mod settings;
pub mod sieve;
pub mod smime;
pub mod smtp;
pub mod spam;
pub mod spell;
pub mod store;
pub mod unsubscribe;
pub mod tags;
pub mod usage;
pub mod vcard;

pub use list::{FolderCount, FolderTree, ListView, Page, PageAt, SortKey, UnreadAt};
pub use model::*;
pub use store::{
    DeleteOutcome, DeleteReport, FolderSyncState, MailStore, StoreError, SyncedMessage, LOCAL_ID,
};
