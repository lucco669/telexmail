//! Isolated security fixtures and mutation targets. This binary is never bundled with Telex.
use std::{collections::HashMap, path::Path, sync::Mutex};
use telex_core::{html, mailbox, mime, MailStore};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("pst-seed") => {
            use pst_fixture::{UnicodePstFile, UnicodePstMessage};
            let path = Path::new(args.get(2).expect("PST output path"));
            assert!(!path.exists(), "Never replace an existing PST file");
            let message = UnicodePstMessage {
                subject: "Security fixture",
                sender_name: "Sender",
                sender_email: "sender@example.invalid",
                recipients: &[],
                body: "Synthetic mail",
                html_body: Some("<p>SAFE_CONTENT</p>"),
                message_id: "<security-pst@example.invalid>",
                delivery_time: 134_042_256_000_000_000,
            };
            drop(UnicodePstFile::create_with_attachments(path, &message, &[]).unwrap());
        }
        Some("fixture") => fixtures(Path::new(args.get(2).expect("fixture directory"))),
        Some("mime") | Some("html") => {
            let raw = std::fs::read(args.get(2).expect("input file")).unwrap();
            let _ = mime::parse_message(&raw, "local/security", None);
            let _ = mime::attachments_of(&raw);
            let _ = html::render(&raw, &HashMap::new(), &html::Paper::Light);
            let text = String::from_utf8_lossy(&raw);
            let _ = html::compose_clean(&text);
            let _ = html::to_text(&text);
        }
        Some("pst") => {
            let _ =
                telex_core::migrate::outlook::read_pst(Path::new(args.get(2).expect("input file")));
        }
        _ => panic!("fixture <directory> | mime|html|pst <input file>"),
    }
}

fn fixtures(directory: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    let db = directory.join("telex.db");
    assert!(
        !db.exists(),
        "Use a fresh fixture directory; never replace a mailbox"
    );
    let mut store = MailStore::open(&db).unwrap();
    let account = telex_core::AccountSetup {
        name: "Security fixtures".into(),
        email: "recipient@example.invalid".into(),
        host: "imap.example.invalid".into(),
        smtp_host: "smtp.example.invalid".into(),
        ..Default::default()
    };
    store.upsert_account(&account).unwrap();
    let folder = store.create_local_folder("Security fixtures").unwrap();
    let store = Mutex::new(store);
    let payloads = [
        ("script", "<script>parent.__TELEX_PWNED=1;parent.__TAURI__.core.invoke('set_flagged',{id:1,flagged:true})</script><p>SAFE_CONTENT</p>"),
        ("events", "<img src=x onerror=\"parent.__TELEX_PWNED=1\"><svg onload=\"parent.__TELEX_PWNED=1\"><script>alert(1)</script></svg><p>SAFE_CONTENT</p>"),
        ("navigation", "<meta http-equiv=refresh content='0;url=https://security.invalid/escape'><base href='https://security.invalid/'><iframe srcdoc='<script>parent.__TELEX_PWNED=1</script>'></iframe><form action='https://security.invalid/post'><input name=secret><button>Send</button></form><object data='file:///etc/passwd'></object><p>SAFE_CONTENT</p>"),
        ("remote", "<style>@import url(https://security.invalid/style)</style><img src='https://security.invalid/pixel'><img src='http://127.0.0.1:18973/pixel'><div style='background:url(https://security.invalid/background);color:red'>SAFE_CONTENT</div>"),
        ("schemes", "<a href='javascript:parent.__TELEX_PWNED=1'>attack</a><a href='data:text/html,attack'>data</a><a href='file:///C:/Windows/win.ini'>file</a><a href='https://example.com'>safe link</a><p>SAFE_CONTENT</p>"),
        ("css-injection", "<div style='background:url(\"cid:\\\");position:fixed;inset:0;background:url(\\\"cid:\")'>SAFE_CONTENT</div>"),
        ("mathml", "<math><mtext><table><mglyph><style><!--</style><img title='--><img src=x onerror=parent.__TELEX_PWNED=1>'></table></mtext></math><p>SAFE_CONTENT</p>"),
    ];
    let mut paths = Vec::new();
    for (name, body) in payloads {
        let path = directory.join(format!("{name}.eml"));
        std::fs::write(&path, format!("From: attacker@example.invalid\r\nTo: recipient@example.invalid\r\nSubject: {name}\r\nMessage-ID: <security-{name}@example.invalid>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{body}")).unwrap();
        paths.push(path);
    }
    mailbox::import_files(&store, &folder, &paths, &|_| {}).unwrap();
    let store = store.lock().unwrap();
    let corpus: Vec<_> = store
        .list(&folder)
        .unwrap()
        .iter()
        .map(|m| serde_json::json!({"id":m.id, "name":m.subject}))
        .collect();
    std::fs::write(
        directory.join("corpus.json"),
        serde_json::to_vec_pretty(&corpus).unwrap(),
    )
    .unwrap();
    println!(
        "Created {} hostile-mail fixtures in {}",
        corpus.len(),
        directory.display()
    );
}
