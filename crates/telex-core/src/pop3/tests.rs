use super::*;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;

use crate::filters::{Action, Condition, Field, Op, Rule};
use crate::model::AccountSetup;

// ---- A fake POP3 server ----

/// What the fake server has and how it behaves.
#[derive(Default)]
struct Fake {
    /// UIDL and message, CRLF line ends.
    mail: Vec<(String, Vec<u8>)>,
    /// CAPA's lines; empty: CAPA isn't known (an old server).
    caps: Vec<&'static str>,
    no_uidl: bool,
    /// Answers STLS; `inject` puts a line after its +OK before TLS would start.
    stls: bool,
    inject: bool,
    /// PASS says the mailbox is locked by another program.
    in_use: bool,
    /// A greeting longer than any server's.
    long_greeting: bool,
    /// Every command, as sent (the password masked).
    commands: Vec<String>,
}

type Shared = Arc<std::sync::Mutex<Fake>>;

fn raw(n: usize, from: &str, subject: &str, body: &str) -> Vec<u8> {
    format!(
        "From: {from}\r\nTo: piotr@poczta.pl\r\nSubject: {subject}\r\n\
         Message-ID: <{n}@poczta.pl>\r\nDate: Thu, 15 Oct 2026 10:{:02}:00 +0200\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n",
        n % 60
    )
    .into_bytes()
}

fn letter(n: usize) -> (String, Vec<u8>) {
    (
        format!("UID-{n:04}"),
        raw(n, "anna@studio.pl", &format!("List {n}"), "Dzień dobry"),
    )
}

async fn start(fake: Fake) -> (SocketAddr, Shared) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let shared: Shared = Arc::new(std::sync::Mutex::new(fake));
    let server = shared.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(serve(socket, server.clone()));
        }
    });
    (addr, shared)
}

async fn serve(socket: TcpStream, shared: Shared) -> std::io::Result<()> {
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let greeting = if shared.lock().unwrap().long_greeting {
        format!("+OK {}\r\n", "x".repeat(20_000))
    } else {
        "+OK Fake POP3 ready\r\n".to_owned()
    };
    write.write_all(greeting.as_bytes()).await?;
    // The mailbox as it was at the login, and what DELE marked in it.
    let mut maildrop: Vec<(String, Vec<u8>)> = Vec::new();
    let mut deleted: HashSet<usize> = HashSet::new();
    let mut logged_in = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(()); // hung up without QUIT: nothing is removed
        }
        let line = line.trim_end_matches(['\r', '\n']).to_owned();
        let (verb, arg) = line.split_once(' ').unwrap_or((&line, ""));
        let verb = verb.to_ascii_uppercase();
        if verb == "AUTH" {
            let offered = {
                let mut s = shared.lock().unwrap();
                s.commands.push(line.clone());
                s.caps.iter().any(|c| c.contains("PLAIN"))
            };
            if !offered || !arg.eq_ignore_ascii_case("PLAIN") {
                write.write_all(b"-ERR not supported\r\n").await?;
                continue;
            }
            write.write_all(b"+ \r\n").await?;
            let mut token = String::new();
            reader.read_line(&mut token).await?;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(token.trim())
                .unwrap_or_default();
            let ok = decoded.ends_with(b"\0secret");
            {
                let mut s = shared.lock().unwrap();
                s.commands.push("(token)".into());
                if ok {
                    logged_in = true;
                    maildrop = s.mail.clone();
                }
            }
            let answer: &[u8] = if ok {
                b"+OK logged in\r\n"
            } else {
                b"-ERR [AUTH] invalid\r\n"
            };
            write.write_all(answer).await?;
            continue;
        }
        let (out, bytes, quit) = {
            let mut quit = false;
            let mut s = shared.lock().unwrap();
            s.commands.push(if verb == "PASS" {
                "PASS ***".to_owned()
            } else {
                line.clone()
            });
            let live = |maildrop: &Vec<(String, Vec<u8>)>, deleted: &HashSet<usize>| {
                (1..=maildrop.len())
                    .filter(|i| !deleted.contains(i))
                    .collect::<Vec<_>>()
            };
            let mut out = String::new();
            let mut bytes: Vec<u8> = Vec::new();
            match verb.as_str() {
                "CAPA" if s.caps.is_empty() => out.push_str("-ERR unknown command\r\n"),
                "CAPA" => {
                    out.push_str("+OK Capability list follows\r\n");
                    for c in &s.caps {
                        out.push_str(c);
                        out.push_str("\r\n");
                    }
                    out.push_str(".\r\n");
                }
                "STLS" if s.stls => {
                    out.push_str("+OK Begin TLS negotiation\r\n");
                    if s.inject {
                        out.push_str("+OK injected\r\n");
                    }
                }
                "STLS" => out.push_str("-ERR not supported\r\n"),
                "USER" => out.push_str("+OK\r\n"),
                "PASS" if s.in_use => out.push_str("-ERR [IN-USE] mailbox locked\r\n"),
                "PASS" if arg == "secret" => {
                    logged_in = true;
                    maildrop = s.mail.clone();
                    out.push_str("+OK logged in\r\n");
                }
                "PASS" => out.push_str("-ERR [AUTH] invalid credentials\r\n"),
                _ if !logged_in && verb != "QUIT" => out.push_str("-ERR log in first\r\n"),
                "UIDL" if s.no_uidl => out.push_str("-ERR no UIDL here\r\n"),
                "UIDL" => {
                    out.push_str("+OK\r\n");
                    for i in live(&maildrop, &deleted) {
                        out.push_str(&format!("{i} {}\r\n", maildrop[i - 1].0));
                    }
                    out.push_str(".\r\n");
                }
                "LIST" => {
                    out.push_str("+OK\r\n");
                    for i in live(&maildrop, &deleted) {
                        out.push_str(&format!("{i} {}\r\n", maildrop[i - 1].1.len()));
                    }
                    out.push_str(".\r\n");
                }
                "RETR" => match arg.parse::<usize>() {
                    Ok(i) if (1..=maildrop.len()).contains(&i) && !deleted.contains(&i) => {
                        bytes.extend_from_slice(b"+OK message follows\r\n");
                        let text = &maildrop[i - 1].1;
                        for line in text.split(|&b| b == b'\n') {
                            if line.is_empty() {
                                continue;
                            }
                            let line = line.strip_suffix(b"\r").unwrap_or(line);
                            if line.first() == Some(&b'.') {
                                bytes.push(b'.'); // byte-stuffing
                            }
                            bytes.extend_from_slice(line);
                            bytes.extend_from_slice(b"\r\n");
                        }
                        bytes.extend_from_slice(b".\r\n");
                    }
                    _ => out.push_str("-ERR no such message\r\n"),
                },
                "DELE" => match arg.parse::<usize>() {
                    Ok(i) if (1..=maildrop.len()).contains(&i) && deleted.insert(i) => {
                        out.push_str("+OK marked\r\n");
                    }
                    _ => out.push_str("-ERR no such message\r\n"),
                },
                "QUIT" => {
                    if logged_in {
                        let gone: HashSet<String> =
                            deleted.iter().map(|i| maildrop[i - 1].0.clone()).collect();
                        s.mail.retain(|(u, _)| !gone.contains(u));
                    }
                    quit = true;
                    out.push_str("+OK bye\r\n");
                }
                _ => out.push_str("-ERR unknown command\r\n"),
            }
            (out, bytes, quit)
        };
        write.write_all(out.as_bytes()).await?;
        write.write_all(&bytes).await?;
        if quit {
            return Ok(());
        }
    }
}

// ---- Helpers ----

fn store_with(keep: Pop3Keep) -> (Mutex<MailStore>, Account) {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Piotr".into(),
            email: "piotr@poczta.pl".into(),
            host: "localhost".into(),
            port: 995,
            username: "piotr".into(),
            pop3: Some(keep),
            ..Default::default()
        })
        .unwrap();
    (Mutex::new(store), account)
}

async fn logged_in(addr: SocketAddr) -> Session<TcpStream> {
    let mut session = Session::new(TcpStream::connect(addr).await.unwrap());
    session.greet().await.unwrap();
    session.capa().await.unwrap();
    session.login("piotr", "secret").await.unwrap();
    session
}

fn no_progress(_: SyncProgress) {}

async fn check(addr: SocketAddr, store: &Mutex<MailStore>, account: &Account) -> SyncReport {
    let mut session = logged_in(addr).await;
    sync_session(&mut session, account, store, &no_progress)
        .await
        .unwrap()
}

fn inbox(store: &Mutex<MailStore>) -> Vec<crate::MessageSummary> {
    store.lock().unwrap().list("poczta/inbox").unwrap()
}

fn on_server(server: &Shared) -> Vec<String> {
    let s = server.lock().unwrap();
    s.mail.iter().map(|(u, _)| u.clone()).collect()
}

// ---- The protocol ----

#[tokio::test]
async fn mail_comes_down_once_by_its_uidl() {
    let mut mail: Vec<_> = (1..=3).map(letter).collect();
    // A line that starts with a dot goes byte-stuffed and comes back as it was.
    mail[1].1 = raw(
        2,
        "anna@studio.pl",
        "Kropki",
        ".kropka na początku\r\n..dwie",
    );
    let original = mail[1].1.clone();
    let (addr, server) = start(Fake {
        mail,
        caps: vec!["USER", "UIDL", "TOP"],
        ..Fake::default()
    })
    .await;
    let (store, account) = store_with(Pop3Keep::default());
    let first = check(addr, &store, &account).await;
    assert_eq!(first.new.len(), 3);
    assert!(first.arrived.is_empty(), "a first check brings old mail");
    assert!(store
        .lock()
        .unwrap()
        .folder_tree()
        .unwrap()
        .counts
        .values()
        .all(|c| c.new_unread == 0));
    let id = inbox(&store)
        .iter()
        .find(|m| m.subject == "Kropki")
        .unwrap()
        .id;
    assert_eq!(
        store.lock().unwrap().raw_of(id).unwrap().unwrap(),
        original,
        "exactly as stored on the server"
    );
    // Nothing twice, and everything stays on the server.
    let again = check(addr, &store, &account).await;
    assert!(again.new.is_empty());
    assert_eq!(on_server(&server).len(), 3);
    assert!(store
        .lock()
        .unwrap()
        .pending_changes(&account.id)
        .unwrap()
        .is_empty());

    server.lock().unwrap().mail.push(letter(4));
    let third = check(addr, &store, &account).await;
    assert_eq!(third.new.len(), 1);
    assert_eq!(third.arrived, third.new, "new mail arrived: rules see it");
    assert_eq!(
        store.lock().unwrap().folder_tree().unwrap().counts[&account.folder_key("inbox")]
            .new_unread,
        1
    );
    assert_eq!(inbox(&store).len(), 4);
    let commands = server.lock().unwrap().commands.clone();
    assert!(
        commands.iter().all(|c| !c.starts_with("DELE")),
        "{commands:?}"
    );
    assert!(commands.contains(&"USER piotr".to_owned()));
}

#[tokio::test]
async fn the_first_check_takes_the_newest_and_offers_the_rest() {
    let n = FIRST_SYNC_LIMIT + 3;
    let (addr, server) = start(Fake {
        mail: (1..=n).map(letter).collect(),
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let (store, account) = store_with(Pop3Keep::default());
    let report = check(addr, &store, &account).await;
    assert_eq!(report.new.len(), FIRST_SYNC_LIMIT);
    let subjects: HashSet<String> = inbox(&store).into_iter().map(|m| m.subject).collect();
    assert!(subjects.contains(&format!("List {n}")) && !subjects.contains("List 1"));
    let missing = |store: &Mutex<MailStore>| -> i64 {
        store
            .lock()
            .unwrap()
            .db
            .query_row(
                "SELECT missing FROM folders WHERE account = 'poczta' AND id = 'inbox'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(missing(&store), 3, "the status bar offers the older three");
    // A new message after that is new mail, not one of the older ones.
    server.lock().unwrap().mail.push(letter(n + 1));
    let next = check(addr, &store, &account).await;
    assert_eq!(next.arrived.len(), 1);
    assert_eq!(missing(&store), 3);

    let mut session = logged_in(addr).await;
    let older = download_older_in(&mut session, &account, &store, 2, &no_progress)
        .await
        .unwrap();
    assert_eq!(older.len(), 2);
    let subjects: HashSet<String> = inbox(&store).into_iter().map(|m| m.subject).collect();
    assert!(subjects.contains("List 3") && subjects.contains("List 2"));
    assert!(
        !subjects.contains("List 1"),
        "the newest of the older first"
    );
    assert_eq!(missing(&store), 1);
}

#[tokio::test]
async fn mail_deleted_here_leaves_the_server_and_never_comes_back() {
    let (addr, server) = start(Fake {
        mail: (1..=3).map(letter).collect(),
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let (store, account) = store_with(Pop3Keep::default());
    check(addr, &store, &account).await;
    let first = inbox(&store)
        .into_iter()
        .find(|m| m.subject == "List 1")
        .unwrap()
        .id;
    {
        let mut s = store.lock().unwrap();
        // To Trash: still here, so still on the server.
        s.delete(first).unwrap();
        assert_eq!(s.folder_of(first).unwrap().as_deref(), Some("poczta/trash"));
    }
    check(addr, &store, &account).await;
    assert_eq!(on_server(&server).len(), 3, "in Trash it is still here");
    store.lock().unwrap().delete(first).unwrap();
    let report = check(addr, &store, &account).await;
    assert!(report.new.is_empty());
    assert_eq!(on_server(&server), ["UID-0002", "UID-0003"]);
    // Moving mail about here changes nothing on the server.
    let second = inbox(&store)[0].id;
    store
        .lock()
        .unwrap()
        .move_to(second, "poczta/archive")
        .unwrap();
    check(addr, &store, &account).await;
    assert_eq!(on_server(&server).len(), 2);
}

#[tokio::test]
async fn without_until_deleted_deleted_mail_stays_there_but_not_here() {
    let (addr, server) = start(Fake {
        mail: (1..=2).map(letter).collect(),
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let keep = Pop3Keep {
        until_deleted: false,
        ..Pop3Keep::default()
    };
    let (store, account) = store_with(keep);
    check(addr, &store, &account).await;
    let id = inbox(&store)[0].id;
    {
        let mut s = store.lock().unwrap();
        s.delete(id).unwrap();
        s.delete(id).unwrap();
    }
    let report = check(addr, &store, &account).await;
    assert!(
        report.new.is_empty(),
        "a UIDL deleted here doesn't come back"
    );
    assert_eq!(on_server(&server).len(), 2);
    assert_eq!(inbox(&store).len(), 1);
}

#[tokio::test]
async fn mail_can_leave_the_server_at_once_or_after_some_days() {
    let (addr, server) = start(Fake {
        mail: (1..=2).map(letter).collect(),
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let keep = Pop3Keep {
        leave: false,
        ..Pop3Keep::default()
    };
    let (store, account) = store_with(keep);
    check(addr, &store, &account).await;
    assert!(on_server(&server).is_empty(), "removed once it is here");
    assert_eq!(inbox(&store).len(), 2);

    let (addr, server) = start(Fake {
        mail: (1..=2).map(letter).collect(),
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let keep = Pop3Keep {
        days: Some(3),
        ..Pop3Keep::default()
    };
    let (store, account) = store_with(keep);
    check(addr, &store, &account).await;
    assert_eq!(on_server(&server).len(), 2);
    // One came five days ago.
    let five_days_ago = Local::now().naive_local() - TimeDelta::days(5);
    store
        .lock()
        .unwrap()
        .db
        .execute(
            "UPDATE pop3_uidls SET downloaded = ?1 WHERE uidl = 'UID-0001'",
            [five_days_ago],
        )
        .unwrap();
    check(addr, &store, &account).await;
    assert_eq!(on_server(&server), ["UID-0002"]);
    assert_eq!(inbox(&store).len(), 2, "still here");
}

#[test]
fn what_stays_follows_the_settings() {
    let now = Local::now().naive_local();
    let listing: Vec<(u32, String)> = ["here", "gone", "old", "older", "known", "big"]
        .iter()
        .enumerate()
        .map(|(i, u)| (i as u32 + 1, (*u).to_owned()))
        .collect();
    let known = |state, message: Option<MessageId>, days: i64| Known {
        state,
        message,
        downloaded: Some(now - TimeDelta::days(days)),
    };
    let all: HashMap<String, Known> = HashMap::from([
        ("here".to_owned(), known(State::Here, Some(1), 0)),
        ("gone".to_owned(), known(State::Here, None, 0)),
        ("old".to_owned(), known(State::Here, Some(3), 30)),
        ("older".to_owned(), known(State::Older, None, 30)),
        ("known".to_owned(), known(State::Known, None, 30)),
        ("big".to_owned(), known(State::Big, None, 30)),
    ]);
    let remove = |keep: Pop3Keep| to_remove(&listing, &all, &keep, now);
    assert_eq!(
        remove(Pop3Keep::default()),
        [2],
        "only what was deleted here"
    );
    assert_eq!(
        remove(Pop3Keep {
            days: Some(14),
            ..Pop3Keep::default()
        }),
        [2, 3, 5]
    );
    assert_eq!(
        remove(Pop3Keep {
            leave: false,
            ..Pop3Keep::default()
        }),
        [1, 2, 3, 5],
        "never what Telex hasn't downloaded"
    );
    assert!(remove(Pop3Keep {
        until_deleted: false,
        ..Pop3Keep::default()
    })
    .is_empty());

    // What comes down: the new ones, oldest first; on a first check only the newest.
    let listing: Vec<(u32, String)> = (1..=5).map(|n| (n, format!("u{n}"))).collect();
    let sizes = HashMap::from([(2, MAX_MESSAGE + 1)]);
    let known = HashMap::from([("u1".to_owned(), known(State::Here, Some(1), 0))]);
    let first = plan(&listing, &sizes, &HashMap::new(), true, 2);
    assert_eq!(first.fetch, [(4, "u4".into()), (5, "u5".into())]);
    assert_eq!(first.older, ["u1", "u3"]);
    assert_eq!(first.big, [("u2".into(), MAX_MESSAGE + 1)]);
    let later = plan(&listing, &sizes, &known, false, 2);
    assert_eq!(later.fetch.len(), 3, "everything new after the first check");
}

#[tokio::test]
async fn stls_comes_before_anything_is_sent() {
    let (addr, server) = start(Fake {
        mail: vec![letter(1)],
        caps: vec!["USER", "UIDL", "STLS"],
        stls: true,
        ..Fake::default()
    })
    .await;
    let mut plain = Session::new(TcpStream::connect(addr).await.unwrap());
    plain.greet().await.unwrap();
    // The fake server stays plain: what matters is the order.
    let stream = plain.stls("localhost").await.unwrap();
    let mut session = Session::new(stream);
    session.capa().await.unwrap();
    assert!(session.caps.uidl && session.caps.user);
    session.login("piotr", "secret").await.unwrap();
    assert_eq!(session.uidls().await.unwrap(), [(1, "UID-0001".to_owned())]);
    let commands = server.lock().unwrap().commands.clone();
    assert_eq!(commands[..4], ["STLS", "CAPA", "USER piotr", "PASS ***"]);

    // A server that won't encrypt never hears the user name or password.
    let (addr, server) = start(Fake {
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let mut plain = Session::new(TcpStream::connect(addr).await.unwrap());
    plain.greet().await.unwrap();
    let refused = plain.stls("localhost").await.err().unwrap();
    assert!(matches!(refused, SyncError::Tls { .. }), "{refused}");
    assert!(refused.to_string().contains("995"), "{refused}");
    assert_eq!(server.lock().unwrap().commands, ["STLS"]);

    // Nor one that slips something in before TLS would start.
    let (addr, _) = start(Fake {
        stls: true,
        inject: true,
        ..Fake::default()
    })
    .await;
    let mut plain = Session::new(TcpStream::connect(addr).await.unwrap());
    plain.greet().await.unwrap();
    assert!(matches!(
        plain.stls("localhost").await,
        Err(SyncError::Tls { .. })
    ));
}

#[tokio::test]
async fn logins_use_auth_plain_where_offered() {
    let (addr, server) = start(Fake {
        caps: vec!["UIDL", "SASL PLAIN LOGIN"],
        ..Fake::default()
    })
    .await;
    let mut session = logged_in(addr).await;
    assert!(session.uidls().await.unwrap().is_empty());
    let commands = server.lock().unwrap().commands.clone();
    assert_eq!(commands[..3], ["CAPA", "AUTH PLAIN", "(token)"]);
    assert!(commands.iter().all(|c| !c.starts_with("USER")));

    // A wrong password is a login error; a mailbox in use isn't.
    let mut session = Session::new(TcpStream::connect(addr).await.unwrap());
    session.greet().await.unwrap();
    session.capa().await.unwrap();
    let wrong = session.login("piotr", "wrong").await.unwrap_err();
    assert!(matches!(wrong, SyncError::Login(_)), "{wrong}");

    let (addr, _) = start(Fake {
        in_use: true,
        ..Fake::default()
    })
    .await;
    let mut session = Session::new(TcpStream::connect(addr).await.unwrap());
    session.greet().await.unwrap();
    session.capa().await.unwrap();
    assert!(!session.caps.answered, "an old server without CAPA");
    let busy = session.login("piotr", "secret").await.unwrap_err();
    assert!(matches!(busy, SyncError::Server(_)), "{busy}");
}

#[tokio::test]
async fn a_server_without_uidl_or_with_huge_lines_is_refused() {
    let (addr, _) = start(Fake {
        caps: vec!["USER"],
        no_uidl: true,
        ..Fake::default()
    })
    .await;
    let mut session = logged_in(addr).await;
    let e = session.uidls().await.unwrap_err();
    assert!(e.to_string().contains("UIDL"), "{e}");

    let (addr, _) = start(Fake {
        long_greeting: true,
        ..Fake::default()
    })
    .await;
    let mut session = Session::new(TcpStream::connect(addr).await.unwrap());
    let e = session.greet().await.unwrap_err();
    assert!(e.to_string().contains("longer"), "{e}");
}

// ---- Arrivals: the same path as IMAP's Inbox ----

#[tokio::test]
async fn new_mail_goes_through_the_rules_and_the_spam_filter() {
    let (addr, server) = start(Fake {
        mail: vec![letter(1)],
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let (store, account) = store_with(Pop3Keep::default());
    check(addr, &store, &account).await;
    let invoices = {
        let mut s = store.lock().unwrap();
        let invoices = s.create_folder(&account.id, "Faktury").unwrap();
        s.save_rule(&Rule {
            id: 0,
            name: "Faktury".into(),
            enabled: true,
            account: Some(account.id.clone()),
            match_all: true,
            conditions: vec![Condition {
                field: Field::Subject,
                op: Op::Contains,
                value: "faktura".into(),
                header: None,
            }],
            actions: vec![Action::Move {
                folder: invoices.clone(),
            }],
            stop: false,
            ..Rule::default()
        })
        .unwrap();
        // The spam filter learns from 50 of each: spam in Spam, read mail in the Inbox.
        for n in 0..50 {
            let spam = raw(
                1000 + n,
                &format!("promo{n}@deal{}.example", n % 7),
                &format!("WIN FREE CASH PRIZE {n}"),
                "casino jackpot bonus winner claim free money now http://win-prize.example/claim",
            );
            let mut m = parse_message(&spam, "poczta/spam", None);
            m.unread = false;
            insert_message(&s.db, "poczta/spam", None, &m, None, Some(&spam)).unwrap();
            let ham = raw(
                2000 + n,
                "marek@klient.pl",
                &format!("Projekt strony {n}"),
                "Dzień dobry, przesyłam poprawki do projektu i harmonogram prac. Pozdrawiam",
            );
            let mut m = parse_message(&ham, "poczta/inbox", None);
            m.unread = false;
            insert_message(&s.db, "poczta/inbox", None, &m, None, Some(&ham)).unwrap();
        }
        while crate::spam::learn_pending(&mut s, crate::spam::LEARN_BATCH).unwrap() > 0 {}
        invoices
    };
    {
        let mut s = server.lock().unwrap();
        s.mail.push((
            "UID-INVOICE".into(),
            raw(
                7,
                "ksiegowa@biuro.pl",
                "Faktura 10/2026",
                "W załączeniu faktura.",
            ),
        ));
        s.mail.push((
            "UID-SPAM".into(),
            raw(
                8,
                "promo99@deal3.example",
                "WIN FREE CASH PRIZE NOW",
                "casino jackpot bonus winner claim free money now http://win-prize.example/claim",
            ),
        ));
    }
    let report = check(addr, &store, &account).await;
    assert_eq!(report.new.len(), 2);
    assert_eq!(report.spam, 1, "{report:?}");
    assert_eq!(report.sorted, 1);
    assert_eq!(
        report.arrived.len(),
        1,
        "what the spam filter caught isn't announced"
    );
    let s = store.lock().unwrap();
    let filed = s.list(&invoices).unwrap();
    assert_eq!(filed.len(), 1);
    assert!(
        s.get(filed[0].id).unwrap().note.is_some(),
        "the rule says why"
    );
    let spam: Vec<_> = s
        .list("poczta/spam")
        .unwrap()
        .into_iter()
        .filter(|m| m.subject == "WIN FREE CASH PRIZE NOW")
        .collect();
    assert_eq!(spam.len(), 1);
    assert!(s.pending_changes(&account.id).unwrap().is_empty());
}

// ---- The account ----

#[test]
fn a_pop3_account_keeps_all_its_folders_here() {
    let (store, account) = store_with(Pop3Keep::default());
    let mut s = store.lock().unwrap();
    let kinds: Vec<FolderKind> = account.folders.iter().map(|f| f.kind).collect();
    assert_eq!(
        kinds,
        [
            FolderKind::Inbox,
            FolderKind::Outbox,
            FolderKind::Drafts,
            FolderKind::Sent,
            FolderKind::Archive,
            FolderKind::Spam,
            FolderKind::Trash
        ]
    );
    assert!(account.folders.iter().all(|f| f.remote.is_none()));
    assert!(account.is_pop3());
    // The user's own folders, as on this PC.
    let folder = s.create_folder(&account.id, "Klienci").unwrap();
    assert_eq!(folder, "poczta/klienci");
    assert!(
        s.create_folder(&account.id, "klienci").is_err(),
        "one of each name"
    );
    s.rename_local_folder(&folder, "Klienci 2026").unwrap();
    assert!(s.rename_local_folder("poczta/inbox", "Poczta").is_err());
    assert_eq!(s.delete_local_folder(&folder).unwrap(), 0);
    // An IMAP account's folders are its server's.
    let imap = s
        .upsert_account(&AccountSetup {
            name: "Studio".into(),
            email: "biuro@twojastudio.pl".into(),
            host: "mail.twojastudio.pl".into(),
            port: 993,
            ..Default::default()
        })
        .unwrap();
    assert!(s.create_folder(&imap.id, "Klienci").is_err());
    // Switching between the two would lose one kind of folders.
    let switch = s.upsert_account(&AccountSetup {
        name: "Piotr".into(),
        email: "piotr@poczta.pl".into(),
        host: "imap.poczta.pl".into(),
        port: 993,
        ..Default::default()
    });
    assert!(switch.unwrap_err().to_string().contains("POP3"));
    // The settings round-trip; updating keeps the account.
    let updated = s
        .upsert_account(&AccountSetup {
            name: "Piotr".into(),
            email: "piotr@poczta.pl".into(),
            host: "pop3.poczta.pl".into(),
            port: 110,
            imap_security: SmtpSecurity::StartTls,
            pop3: Some(Pop3Keep {
                leave: false,
                ..Pop3Keep::default()
            }),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(updated.id, account.id);
    let settings = updated.imap.unwrap();
    assert_eq!(settings.protocol(), "pop3");
    assert_eq!(settings.port, 110);
    assert!(!settings.pop3.unwrap().leave);
}

#[test]
fn sent_mail_goes_to_the_local_sent_and_nothing_is_uploaded() {
    let (store, account) = store_with(Pop3Keep::default());
    let mut s = store.lock().unwrap();
    let message = parse_message(&raw(1, "piotr@poczta.pl", "Oferta", "Treść"), "", None);
    let id = s
        .queue_outgoing(
            &account.id,
            &message,
            b"raw",
            "piotr@poczta.pl",
            &["anna@studio.pl".to_owned()],
            None,
            None,
        )
        .unwrap();
    s.outgoing_sent(id, Local::now().naive_local()).unwrap();
    assert_eq!(s.folder_of(id).unwrap().as_deref(), Some("poczta/sent"));
    assert!(s.pending_changes(&account.id).unwrap().is_empty());
}

#[test]
fn thunderbird_s_record_marks_mail_already_downloaded() {
    let (store, account) = store_with(Pop3Keep::default());
    let mut s = store.lock().unwrap();
    // The message that came along carries its UIDL in an X-UIDL header.
    let with_uidl = b"X-UIDL: UID-0001\r\nFrom: anna@studio.pl\r\nSubject: Z Thunderbirda\r\n\r\nTre\xc5\x9b\xc4\x87\r\n";
    let m = parse_message(with_uidl, "poczta/inbox", None);
    let linked = insert_message(&s.db, "poczta/inbox", None, &m, None, Some(with_uidl)).unwrap();
    let at = Some(Local::now().naive_local());
    let added = s
        .pop3_mark_known(
            &account.id,
            &[
                ("UID-0001".into(), at, false),
                ("UID-0002".into(), at, false),
                ("UID-0003".into(), at, true),
            ],
            Local::now().date_naive(),
        )
        .unwrap();
    assert_eq!(added, 3);
    let known = s.pop3_known(&account.id).unwrap();
    assert_eq!(known["UID-0001"].message, Some(linked));
    assert_eq!(known["UID-0001"].state, State::Here);
    assert_eq!(known["UID-0002"].state, State::Known);
    assert_eq!(
        (known["UID-0003"].state, known["UID-0003"].message),
        (State::Here, None),
        "deleted in Thunderbird: to be removed from the server"
    );
    // Again: nothing twice.
    let again = s
        .pop3_mark_known(
            &account.id,
            &[("UID-0002".into(), at, false)],
            Local::now().date_naive(),
        )
        .unwrap();
    assert_eq!(again, 0);
}

#[tokio::test]
async fn after_moving_in_only_new_mail_comes_down() {
    let (addr, server) = start(Fake {
        mail: (1..=4).map(letter).collect(),
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let (store, account) = store_with(Pop3Keep::default());
    {
        let mut s = store.lock().unwrap();
        let at = Some(Local::now().naive_local());
        s.pop3_mark_known(
            &account.id,
            &[
                ("UID-0001".into(), at, false),
                ("UID-0002".into(), at, false),
                ("UID-0003".into(), at, true),
            ],
            Local::now().date_naive(),
        )
        .unwrap();
    }
    let report = check(addr, &store, &account).await;
    assert_eq!(report.new.len(), 1, "only what Thunderbird hadn't");
    assert_eq!(inbox(&store)[0].subject, "List 4");
    assert_eq!(
        on_server(&server),
        ["UID-0001", "UID-0002", "UID-0004"],
        "what Thunderbird deleted goes"
    );
}

#[test]
fn x_uidl_is_read_from_the_headers_only() {
    assert_eq!(
        x_uidl(b"Subject: a\r\nx-uidl:  abc123 \r\n\r\nbody").as_deref(),
        Some("abc123")
    );
    assert_eq!(x_uidl(b"Subject: a\r\n\r\nX-UIDL: in-the-body\r\n"), None);
}

#[tokio::test]
async fn after_a_restore_mail_from_since_the_backup_comes_down_as_old_mail() {
    let (addr, server) = start(Fake {
        mail: vec![letter(1)],
        caps: vec!["USER", "UIDL"],
        ..Fake::default()
    })
    .await;
    let (store, account) = store_with(Pop3Keep::default());
    check(addr, &store, &account).await;
    // The backup knew of one message; the server has another since.
    server.lock().unwrap().mail.push(letter(2));
    store.lock().unwrap().after_restore().unwrap();
    let report = check(addr, &store, &account).await;
    assert_eq!(report.new.len(), 1);
    assert!(
        report.arrived.is_empty(),
        "no rules, replies or notifications twice"
    );
    server.lock().unwrap().mail.push(letter(3));
    assert_eq!(check(addr, &store, &account).await.arrived.len(), 1);
}

/// TLS on 995 and a real server's greeting and CAPA (rustls with Mozilla's roots). No login.
#[tokio::test]
#[ignore = "needs the internet"]
async fn tls_reaches_a_real_server() {
    let settings = ImapSettings {
        host: "pop.gmail.com".into(),
        port: PORT_TLS,
        username: String::new(),
        security: SmtpSecurity::Tls,
        pop3: Some(Pop3Keep::default()),
    };
    let mut trace = Trace::new(Purpose::Test, "pop3", "", "pop.gmail.com", PORT_TLS);
    let mut session = connect(&settings, "pop.gmail.com", &mut trace)
        .await
        .unwrap();
    session.capa().await.unwrap();
    assert!(session.caps.uidl, "{:?}", session.caps);
    let _ = session.quit().await;
}
