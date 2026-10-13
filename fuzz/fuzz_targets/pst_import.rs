#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;
use telex_core::{migrate::{self, outlook}, MailStore, AccountSetup};
use std::sync::Mutex;

fuzz_target!(|data: &[u8]| {
    // One libFuzzer process, one private file. No production paths or mailbox.
    let path: PathBuf = std::env::temp_dir().join(format!("telex-pst-fuzz-{}.pst", std::process::id()));
    std::fs::write(&path, data).unwrap();
    if let Ok(source) = outlook::read_pst(&path) {
        let mut store=MailStore::in_memory().unwrap();
        store.upsert_account(&AccountSetup{name:"Fuzzer".into(),email:"fuzzer@example.invalid".into(),host:"imap.example.invalid".into(),..Default::default()}).unwrap();
        let keys=source.folders.iter().map(|f| f.key.clone()).collect::<Vec<_>>();
        let _=migrate::import_mail(&Mutex::new(store),&source,&keys,&|_|{});
    }
    std::fs::remove_file(path).unwrap();
});
