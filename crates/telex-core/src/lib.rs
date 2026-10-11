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
pub mod backup;
mod colors;
pub mod drafts;
pub mod filters;
pub mod html;
pub mod i18n;
pub mod imap;
pub mod list;
pub mod mailbox;
pub mod migrate;
pub mod mime;
pub mod model;
pub mod recipients;
pub mod sample;
pub mod settings;
pub mod smtp;
pub mod store;
pub mod usage;

pub use list::{FolderCount, FolderTree, ListView, Page, PageAt, SortKey, UnreadAt};
pub use model::*;
pub use store::{
    DeleteOutcome, DeleteReport, FolderSyncState, MailStore, StoreError, SyncedMessage, LOCAL_ID,
};
