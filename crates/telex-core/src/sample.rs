//! Built-in sample mailbox, shown until the user adds a real account.
//! The data lives in `sample.json` next to this file.

use serde::Deserialize;

use crate::model::{Account, Message};
use crate::store::MailStore;

#[derive(Deserialize)]
struct SampleFile {
    accounts: Vec<Account>,
    messages: Vec<Message>,
    /// Messages that arrive the first time the user presses "Get mail".
    #[serde(default)]
    incoming: Vec<Message>,
}

const SAMPLE_JSON: &str = include_str!("sample.json");

/// An in-memory store filled with the sample mailbox: two accounts and about thirty messages.
pub fn sample_store() -> MailStore {
    let file: SampleFile =
        serde_json::from_str(SAMPLE_JSON).expect("sample.json is part of the build and must parse");
    MailStore::new(file.accounts, file.messages, file.incoming)
        .expect("an in-memory database always opens")
}
