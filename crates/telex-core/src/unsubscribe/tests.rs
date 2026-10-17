use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use super::*;
use crate::i18n::{self, Lang};
use crate::model::{AccountSetup, Folder};
use crate::store::SyncedMessage;

fn mail(to: &str, subject: &str, body: &str) -> Method {
    Method::Mail {
        to: to.into(),
        subject: subject.into(),
        body: body.into(),
    }
}

#[test]
fn addresses_in_brackets_comments_and_folded_lines() {
    assert_eq!(
        urls("<mailto:list@host.com?subject=unsubscribe>, (Use this to get off the list) <https://example.com/unsub?id=1>"),
        ["mailto:list@host.com?subject=unsubscribe", "https://example.com/unsub?id=1"]
    );
    // A folded line leaves spaces inside the brackets; a comment may hold brackets of its own.
    assert_eq!(
        urls("(see <this>) <https://news.sklep.pl/u/\r\n  abc123>,\r\n <mailto:u@sklep.pl>"),
        ["https://news.sklep.pl/u/abc123", "mailto:u@sklep.pl"]
    );
    assert_eq!(
        urls("<https://a.pl/(x)>"),
        ["https://a.pl/(x)"],
        "parentheses inside an address are its own"
    );
    // Without brackets, as some senders write it.
    assert_eq!(
        urls("mailto:u@x.pl, https://x.pl/u, nonsense"),
        ["mailto:u@x.pl", "https://x.pl/u"]
    );
    assert!(urls("").is_empty());
    assert!(urls("<>").is_empty());
}

#[test]
fn one_click_over_https_first_then_mail_then_a_page() {
    let header = "<mailto:u@sklep.pl?subject=wypisz>, <https://sklep.pl/u?t=1>";
    let post = Some("List-Unsubscribe=One-Click");
    assert_eq!(
        choose(header, post),
        Some(Method::OneClick("https://sklep.pl/u?t=1".into()))
    );
    assert!(one_click(Some(" list-unsubscribe = one-click ")));
    assert!(!one_click(Some("List-Unsubscribe=Later")));
    // Without List-Unsubscribe-Post the address is only a page: mail goes first.
    assert_eq!(
        choose(header, None),
        Some(mail("u@sklep.pl", "wypisz", "unsubscribe"))
    );
    // One click needs https: plain http is a page at most, and mail still wins.
    assert_eq!(
        choose("<http://sklep.pl/u>, <mailto:u@sklep.pl>", post),
        Some(mail("u@sklep.pl", "unsubscribe", "unsubscribe"))
    );
    assert_eq!(
        choose("<http://sklep.pl/u>, <https://sklep.pl/u2>", None),
        Some(Method::Link("https://sklep.pl/u2".into())),
        "https before http"
    );
    assert_eq!(
        choose("<http://sklep.pl/u>", post),
        Some(Method::Link("http://sklep.pl/u".into()))
    );
    assert_eq!(choose("<ftp://x.pl/u>, <javascript:alert(1)>", post), None);
    assert_eq!(choose("<https:///nohost>", post), None);
}

#[test]
fn mailto_subject_and_text_are_decoded() {
    assert_eq!(
        choose(
            "<mailto:lista%2Bwypisz@x.pl?Subject=Wypisz%20mnie%20%C5%82adnie&body=STOP>",
            None
        ),
        Some(mail("lista+wypisz@x.pl", "Wypisz mnie ładnie", "STOP"))
    );
    // Only the first of several addresses, and no header injection through the subject.
    assert_eq!(
        choose("<mailto:a@x.pl,b@x.pl>", None),
        Some(mail("a@x.pl", "unsubscribe", "unsubscribe"))
    );
    assert_eq!(
        choose("<mailto:a@x.pl?subject=x%0D%0ABcc:%20all@x.pl>", None),
        None
    );
    assert_eq!(choose("<mailto:?subject=x>", None), None);
    assert_eq!(decoded("100%"), "100%");
    assert_eq!(decoded("%zz%4"), "%zz%4");
}

#[test]
fn what_the_user_is_shown() {
    assert_eq!(
        Method::OneClick("https://User@News.Sklep.pl:8443/u?x".into()).target(),
        "news.sklep.pl"
    );
    assert_eq!(Method::Link("https://[::1]:80/u".into()).target(), "[::1]");
    assert_eq!(mail("u@sklep.pl", "s", "b").target(), "u@sklep.pl");
    assert_eq!(Method::OneClick(String::new()).code(), "one_click");
}

#[test]
fn the_list_is_known_by_its_id_or_its_sender() {
    let raw =
        b"From: News <news@sklep.pl>\r\nList-Id: Nowosci Sklepu\r\n <Nowosci.Sklep.pl>\r\n\r\nHej";
    assert_eq!(list_key(Some(raw), "news@sklep.pl"), "nowosci.sklep.pl");
    assert_eq!(
        list_key(Some(b"From: a@b.pl\r\n\r\n"), "News@Sklep.PL"),
        "news@sklep.pl"
    );
    assert_eq!(list_key(None, "x@y.pl"), "x@y.pl");
    let raw = b"List-Unsubscribe: <https://sklep.pl/u?t=1>,\r\n <mailto:u@sklep.pl>\r\nList-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n\r\n";
    assert_eq!(
        method_of(raw),
        Some(Method::OneClick("https://sklep.pl/u?t=1".into()))
    );
    assert_eq!(method_of(b"Subject: x\r\n\r\n"), None);
}

#[test]
fn one_click_never_reaches_this_computer_or_the_local_network() {
    for url in [
        "https://127.0.0.1:1/u",
        "https://localhost:1/u",
        "https://[::1]:1/u",
        "https://192.168.1.1/u",
        "https://10.0.0.1/u",
        "https://169.254.169.254/latest",
    ] {
        let error = post(url, false).unwrap_err();
        assert!(
            error.contains("your own network"),
            "{url} must be refused before connecting: {error}"
        );
    }
    let error = post("http://sklep.pl/u", false).unwrap_err();
    assert!(error.contains("only over https"), "{error}");
    let error = i18n::with(Lang::Pl, || post("ftp://sklep.pl/u", false).unwrap_err());
    assert_eq!(
        error,
        "Telex wypisuje jednym kliknięciem tylko przez https."
    );
}

/// A web server that answers one request with `answer` and hands back what it was sent.
fn one_request(answer: &'static str) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        // The head, then as much body as Content-Length says.
        loop {
            let n = socket.read(&mut buf).unwrap();
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if got.len() >= end + 4 + length {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        socket.write_all(answer.as_bytes()).unwrap();
        tx.send(String::from_utf8_lossy(&got).to_string()).unwrap();
    });
    (port, rx)
}

#[test]
fn one_click_posts_only_the_request_and_follows_no_redirect() {
    let (port, sent) =
        one_request("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    post(&format!("http://127.0.0.1:{port}/u?t=abc"), true).unwrap();
    let request = sent.recv().unwrap();
    let lower = request.to_ascii_lowercase();
    assert!(
        request.starts_with("POST /u?t=abc HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(lower.contains("content-type: application/x-www-form-urlencoded"));
    assert!(
        lower.contains("user-agent: mozilla/5.0\r\n"),
        "a generic one: {request}"
    );
    assert!(!lower.contains("cookie:") && !lower.contains("referer:"));
    assert!(
        request.ends_with("\r\n\r\nList-Unsubscribe=One-Click"),
        "{request}"
    );

    let (port, _sent) = one_request(
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let error = post(&format!("http://127.0.0.1:{port}/u"), true).unwrap_err();
    assert!(error.contains("answered 302"), "{error}");
}

fn store_with_list_mail() -> (MailStore, String) {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Studio".into(),
            email: "biuro@twojastudio.pl".into(),
            host: "mail.twojastudio.pl".into(),
            port: 993,
            ..Default::default()
        })
        .unwrap();
    let folder = |id: &str, kind| Folder {
        id: id.into(),
        name: id.into(),
        kind,
        remote: Some(id.to_uppercase()),
    };
    store
        .set_folders(
            &account.id,
            &[
                folder("inbox", FolderKind::Inbox),
                folder("spam", FolderKind::Spam),
                folder("sent", FolderKind::Sent),
            ],
        )
        .unwrap();
    (store, account.id)
}

fn deliver(
    store: &mut MailStore,
    account: &str,
    folder: &str,
    uid: u32,
    headers: &str,
) -> MessageId {
    let raw = format!(
        "Message-ID: <n{uid}@sklep.pl>\r\nFrom: Sklep <news@sklep.pl>\r\nTo: biuro@twojastudio.pl\r\nSubject: Nowości\r\n{headers}\r\nHej\r\n"
    );
    let key = format!("{account}/{folder}");
    let mut message = crate::mime::parse_message(raw.as_bytes(), &key, None);
    message.folder = key.clone();
    store
        .add_synced(
            &key,
            &[SyncedMessage {
                uid,
                message,
                raw: raw.into_bytes(),
            }],
        )
        .unwrap()[0]
}

#[test]
fn offered_outside_spam_and_remembered_for_the_whole_list() {
    let (mut store, account) = store_with_list_mail();
    let list =
        "List-Id: <nowosci.sklep.pl>\r\nList-Unsubscribe: <mailto:u@sklep.pl?subject=wypisz>\r\n";
    let first = deliver(&mut store, &account, "inbox", 1, list);
    let second = deliver(&mut store, &account, "inbox", 2, list);
    let plain = deliver(&mut store, &account, "inbox", 3, "");
    let in_spam = deliver(&mut store, &account, "spam", 4, list);

    let offer = store.inspect_message(first).unwrap().unsubscribe.unwrap();
    assert_eq!(
        offer,
        Offer {
            method: "mail",
            target: "u@sklep.pl".into(),
            done: None,
            spam: false
        }
    );
    assert!(store.inspect_message(plain).unwrap().unsubscribe.is_none());
    assert!(store.unsubscribe_plan(plain).is_err());
    // In Spam the button is there to say why it does nothing; the plan refuses.
    assert!(
        store
            .inspect_message(in_spam)
            .unwrap()
            .unsubscribe
            .unwrap()
            .spam
    );
    let error = store.unsubscribe_plan(in_spam).unwrap_err().to_string();
    assert!(error.contains("Spam"), "{error}");

    let plan = store.unsubscribe_plan(first).unwrap();
    assert_eq!(plan.list, "nowosci.sklep.pl");
    assert_eq!(plan.method, mail("u@sklep.pl", "wypisz", "unsubscribe"));
    let (from, identity) = store.unsubscribe_sender(first).unwrap();
    assert_eq!(from, account);
    let me = store.account(&account).unwrap().identity(None);
    assert_eq!(identity, me.id);
    let d = draft(&from, Some(identity), &plan.method).unwrap();
    assert_eq!(
        (d.to.as_str(), d.subject.as_str(), d.body.as_str()),
        ("u@sklep.pl", "wypisz", "unsubscribe")
    );
    assert!(draft(&from, None, &Method::Link("https://x.pl".into())).is_none());

    let at = chrono::NaiveDate::from_ymd_opt(2026, 10, 17)
        .unwrap()
        .and_hms_opt(9, 30, 0)
        .unwrap();
    i18n::with(Lang::Pl, || store.record_unsubscribe(&plan, at)).unwrap();
    assert_eq!(
        store.get(first).unwrap().note.as_deref(),
        Some("Wypisano mailem do u@sklep.pl 17.10.2026.")
    );
    // The next message of the list knows, without a note of its own.
    let next = store.inspect_message(second).unwrap();
    assert_eq!(next.unsubscribe.unwrap().done, Some(at));
    assert!(next.message.note.is_none());
    // Twice: the note isn't repeated.
    i18n::with(Lang::Pl, || store.record_unsubscribe(&plan, at)).unwrap();
    assert_eq!(
        store.get(first).unwrap().note.as_deref(),
        Some("Wypisano mailem do u@sklep.pl 17.10.2026.")
    );
    let one_click = Plan {
        method: Method::OneClick("https://sklep.pl/u".into()),
        ..plan
    };
    store.record_unsubscribe(&one_click, at).unwrap();
    assert!(store
        .get(first)
        .unwrap()
        .note
        .unwrap()
        .ends_with("Unsubscribed with one click on 17.10.2026."));
}
