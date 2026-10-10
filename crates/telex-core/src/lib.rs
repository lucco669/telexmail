//! Telex mail engine.
//!
//! Everything that is not UI lives here, so it can be tested with plain
//! `cargo test -p telex-core` and without a window. The Tauri app in
//! `src-tauri` is a thin layer that exposes these functions to the UI.
//!
//! Today the store holds a built-in sample mailbox in memory. The plan is to
//! keep the `MailStore` API and swap its insides for SQLite + IMAP sync.

pub mod model;
pub mod sample;
pub mod store;

pub use model::*;
pub use store::{DeleteOutcome, MailStore, Snapshot, StoreError};
