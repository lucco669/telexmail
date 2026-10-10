//! Telex mail engine.
//!
//! Everything that is not UI lives here, so it can be tested with plain
//! `cargo test -p telex-core` and without a window. The Tauri app in
//! `src-tauri` is a thin layer that exposes these functions to the UI.
//!
//! `MailStore` is the local mail base in SQLite. `imap` fills it from a server,
//! read-only for now; `mime` turns raw messages into `Message`s.

pub mod imap;
pub mod mime;
pub mod model;
pub mod sample;
pub mod store;

pub use model::*;
pub use store::{DeleteOutcome, FolderSyncState, MailStore, Snapshot, StoreError, SyncedMessage};
