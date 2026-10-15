//! Speed at scale: a synthetic mail base of 100,000 messages in three accounts, and how long
//! the things the window does take on it. Ignored by default, because building the mail base
//! takes a minute or two. Run it in release mode, as Telex ships:
//!
//!   cargo test -p telex-core --release --test scale -- --ignored --nocapture
//!
//! TELEX_SCALE_DB=<path> keeps the mail base in that file and reuses it on the next run.
//! TELEX_SCALE_V6=<path> also times upgrading a version 6 mail base (on a copy).
//! TELEX_SCALE_EXPORT=<directory> exports the same messages as mbox for other clients.
//! The numbers are recorded in docs/storage.md.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use telex_core::filters::{self, Action, Condition, Field, Op, Rule};
use telex_core::{
    AccountSetup, Attachment, Folder, FolderKind, ListView, MailStore, Message, MessageId, PageAt,
    SortKey, SyncedMessage,
};

/// A folder's id, what it is for, and how many messages it holds.
type FolderPlan = (&'static str, FolderKind, usize);

/// Accounts, their folders and how many messages each holds: 100,000 in all.
const LAYOUT: &[(&str, &str, &[FolderPlan])] = &[
    (
        "Studio",
        "biuro@twojastudio.pl",
        &[
            ("inbox", FolderKind::Inbox, 40_000),
            ("sent", FolderKind::Sent, 12_000),
            ("klienci", FolderKind::Custom, 6_000),
            ("archive", FolderKind::Archive, 15_000),
            ("spam", FolderKind::Spam, 2_000),
            ("trash", FolderKind::Trash, 3_000),
        ],
    ),
    (
        "Gmail",
        "lukasz.kowalski@gmail.com",
        &[
            ("inbox", FolderKind::Inbox, 12_000),
            ("sent", FolderKind::Sent, 3_000),
            ("archive", FolderKind::Archive, 2_000),
            ("trash", FolderKind::Trash, 1_000),
        ],
    ),
    (
        "Dom",
        "lukasz@kowalscy.pl",
        &[
            ("inbox", FolderKind::Inbox, 3_500),
            ("sent", FolderKind::Sent, 500),
        ],
    ),
];

const FIRST: &[&str] = &[
    "Anna",
    "Piotr",
    "Katarzyna",
    "Tomasz",
    "Małgorzata",
    "Paweł",
    "Agnieszka",
    "Michał",
    "Joanna",
    "Krzysztof",
    "Ewa",
    "Łukasz",
    "Zofia",
    "Jakub",
    "Magdalena",
    "Wojciech",
    "Sarah",
    "James",
    "Emma",
    "Oliver",
    "Żaneta",
    "Grzegorz",
    "Dorota",
    "Marek",
    "Beata",
    "Jan",
];
const LAST: &[&str] = &[
    "Kowalska",
    "Nowak",
    "Wiśniewski",
    "Wójcik",
    "Kamińska",
    "Lewandowski",
    "Zielińska",
    "Szymański",
    "Woźniak",
    "Dąbrowski",
    "Kozłowska",
    "Jankowski",
    "Mazur",
    "Krawczyk",
    "Piotrowska",
    "Grabowski",
    "Carter",
    "Miller",
    "Brown",
    "Żak",
    "Sikora",
    "Baran",
];
const DOMAINS: &[&str] = &[
    "kowalska-architekci.pl",
    "nowak-meble.pl",
    "gmail.com",
    "wp.pl",
    "o2.pl",
    "onet.pl",
    "interia.pl",
    "outlook.com",
    "firma.com.pl",
    "lindenstudio.com",
    "biuro-rachunkowe.pl",
];
/// Words for subjects and bodies, the most frequent first.
const WORDS: &[&str] = &[
    "w",
    "i",
    "na",
    "się",
    "do",
    "nie",
    "z",
    "że",
    "to",
    "jest",
    "projekt",
    "dzień",
    "dobry",
    "proszę",
    "o",
    "jak",
    "przesyłam",
    "faktura",
    "wycena",
    "termin",
    "spotkanie",
    "pozdrawiam",
    "dziękuję",
    "plik",
    "załącznik",
    "zmiany",
    "poprawki",
    "strona",
    "klient",
    "umowa",
    "wrzesień",
    "październik",
    "listopad",
    "środa",
    "czwartek",
    "piątek",
    "poniedziałek",
    "kosztorys",
    "projektu",
    "budżet",
    "realizacja",
    "wdrożenie",
    "serwer",
    "domena",
    "hosting",
    "logo",
    "grafika",
    "makieta",
    "akceptacja",
    "zamówienie",
    "dostawa",
    "płatność",
    "przelew",
    "rachunek",
    "zaliczka",
    "kontakt",
    "telefon",
    "adres",
    "biuro",
    "Kraków",
    "Warszawa",
    "Gdańsk",
    "Wrocław",
    "Poznań",
    "Łódź",
    "szkic",
    "wersja",
    "druk",
    "katalog",
    "ulotka",
    "baner",
    "zdjęcia",
    "sesja",
    "tekst",
    "korekta",
    "tłumaczenie",
    "newsletter",
    "kampania",
    "reklama",
    "budowa",
    "remont",
    "kuchnia",
    "łazienka",
    "meble",
    "szafa",
    "blat",
    "drewno",
    "dąb",
    "orzech",
    "lakier",
    "montaż",
    "pomiar",
    "wymiary",
    "rysunek",
    "wizualizacja",
    "the",
    "and",
    "invoice",
    "meeting",
    "deadline",
    "review",
    "update",
    "proposal",
    "draft",
    "schedule",
    "budget",
    "launch",
    "website",
    "design",
    "feedback",
    "approval",
    "contract",
    "delivery",
    "payment",
    "question",
    "answer",
    "tomorrow",
    "today",
    "week",
    "month",
    "quarter",
    "report",
    "summary",
    "notes",
    "agenda",
    "call",
    "zoom",
    "office",
    "team",
];
const SUBJECTS: &[&str] = &[
    "Wycena",
    "Faktura",
    "Poprawki na stronie",
    "Spotkanie w czwartek",
    "Projekt kuchni",
    "Umowa do podpisu",
    "Zdjęcia z sesji",
    "Newsletter październik",
    "Makieta strony głównej",
    "Termin realizacji",
    "Pytanie o dostawę",
    "Invoice",
    "Meeting notes",
    "Website review",
    "Proposal draft",
    "Kosztorys remontu",
    "Przelew",
    "Logo – wersja 3",
    "Akceptacja projektu",
];

/// A small, fixed random number generator, so every run builds the same mail base.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// Index into a list, favoring the start: like word frequencies.
    fn skewed(&mut self, n: usize) -> usize {
        let u = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        ((u * u) * n as f64) as usize
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(1000) < percent * 10
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

fn person(rng: &mut Rng) -> (String, String) {
    let (first, last) = (rng.pick(FIRST), rng.pick(LAST));
    let ascii = |s: &str| {
        s.to_lowercase()
            .replace('ą', "a")
            .replace('ć', "c")
            .replace('ę', "e")
            .replace('ł', "l")
            .replace('ń', "n")
            .replace('ó', "o")
            .replace('ś', "s")
            .replace(['ź', 'ż'], "z")
    };
    let email = format!("{}.{}@{}", ascii(first), ascii(last), rng.pick(DOMAINS));
    (format!("{first} {last}"), email)
}

fn text(rng: &mut Rng, words: usize) -> String {
    let mut out = String::new();
    for i in 0..words {
        if i > 0 {
            out.push(if i % 14 == 0 { '\n' } else { ' ' });
        }
        out.push_str(WORDS[rng.skewed(WORDS.len())]);
    }
    out
}

/// A base64-looking line block of about `kb` kilobytes, for attachments in the raw message.
fn blob(rng: &mut Rng, kb: usize) -> String {
    const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(kb * 1024 + kb * 14);
    while out.len() < kb * 1024 {
        for _ in 0..76 {
            out.push(B64[rng.below(64)] as char);
        }
        out.push_str("\r\n");
    }
    out
}

fn message(
    rng: &mut Rng,
    owner: (&str, &str),
    kind: FolderKind,
    uid: u32,
    date: NaiveDateTime,
    id_part: &str,
) -> SyncedMessage {
    let (contact_name, contact_email) = person(rng);
    let outgoing = kind.is_outgoing();
    let (from_name, from_email, to) = if outgoing {
        (
            owner.0.to_owned(),
            owner.1.to_owned(),
            format!("{contact_name} <{contact_email}>"),
        )
    } else {
        (contact_name, contact_email, owner.1.to_owned())
    };
    let prefix = ["", "", "", "Re: ", "Odp: ", "Fwd: ", "RE: Re: "][rng.below(7)];
    let (topic, words) = (rng.pick(SUBJECTS), 1 + rng.below(4));
    let subject = format!("{prefix}{topic} {}", text(rng, words));
    let words = 60 + rng.below(340);
    let mut body = format!(
        "Dzień dobry,\n\n{}\n\nPozdrawiam\n{from_name}",
        text(rng, words)
    );
    // A rare word, for searches with few hits.
    if rng.chance(1) {
        body.push_str("\n\nPS. Przesyłam zdjęcie żółwia.");
    }
    let attachments = if rng.chance(10) {
        vec![Attachment {
            name: format!("{}.pdf", text(rng, 1)),
            size_kb: 20 + rng.below(60) as u32,
            path: None,
        }]
    } else {
        Vec::new()
    };
    let message_id = format!("{uid}.{id_part}@bench.telex");
    // Replies answer the message before them in the folder: conversations of a few messages.
    let refs = if !prefix.is_empty() && uid > 1 {
        vec![format!("{}.{id_part}@bench.telex", uid - 1)]
    } else {
        Vec::new()
    };
    let in_reply_to = refs
        .first()
        .map(|r| format!("In-Reply-To: <{r}>\r\n"))
        .unwrap_or_default();
    let html = body
        .split("\n\n")
        .map(|p| {
            format!(
                "<p style=\"margin:0 0 12px\">{}</p>",
                p.replace('\n', "<br>")
            )
        })
        .collect::<String>();
    let mut raw = format!(
        "From: {from_name} <{from_email}>\r\nTo: {to}\r\nSubject: {subject}\r\n\
         Date: {}\r\nMessage-ID: <{message_id}>\r\n{in_reply_to}MIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n\
         --b1\r\nContent-Type: multipart/alternative; boundary=\"b2\"\r\n\r\n\
         --b2\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n\
         --b2\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body>{html}</body></html>\r\n--b2--\r\n",
        date.format("%a, %d %b %Y %H:%M:%S +0200"),
    );
    for a in &attachments {
        raw.push_str(&format!(
            "--b1\r\nContent-Type: application/pdf; name=\"{}\"\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n{}",
            a.name,
            blob(rng, a.size_kb as usize)
        ));
    }
    raw.push_str("--b1--\r\n");
    let size_kb = u32::try_from(raw.len().div_ceil(1024)).unwrap_or(u32::MAX);
    SyncedMessage {
        uid,
        message: Message {
            id: 0,
            folder: String::new(),
            from_name,
            from_email,
            to,
            cc: String::new(),
            subject,
            date,
            size_kb,
            unread: !outgoing && rng.chance(3),
            flagged: rng.chance(1),
            remote_images: false,
            note: None,
            attachments,
            body,
            message_id: Some(message_id),
            bcc: String::new(),
            refs,
        },
        raw: raw.into_bytes(),
    }
}

/// Fill an empty mail base the way sync does, in batches of 1000.
fn build(store: &mut MailStore) {
    let mut rng = Rng(0x7e1e_c0de_2026_1010);
    let start = NaiveDate::from_ymd_opt(2021, 10, 10)
        .unwrap()
        .and_hms_opt(8, 0, 0)
        .unwrap();
    let span = 5 * 365 * 24 * 3600_i64;
    for (name, email, folders) in LAYOUT {
        let account = store
            .upsert_account(&AccountSetup {
                name: (*name).into(),
                email: (*email).into(),
                host: format!("mail.{}", email.split('@').nth(1).unwrap()),
                port: 993,
                username: (*email).into(),
                ..Default::default()
            })
            .unwrap();
        let list: Vec<Folder> = folders
            .iter()
            .map(|(id, kind, _)| Folder {
                id: (*id).into(),
                name: (*id).into(),
                kind: *kind,
                remote: Some(id.to_uppercase()),
            })
            .collect();
        store.set_folders(&account.id, &list).unwrap();
        for (folder, kind, count) in *folders {
            let key = account.folder_key(folder);
            let id_part = key.replace('/', ".");
            let mut batch = Vec::with_capacity(1000);
            for i in 0..*count {
                // Dates rise with the UID, as on a server, over five years.
                let secs = span * i as i64 / *count as i64 + rng.below(3600) as i64;
                let date = start + TimeDelta::seconds(secs);
                let uid = i as u32 + 1;
                batch.push(message(&mut rng, (name, email), *kind, uid, date, &id_part));
                if batch.len() == 1000 || i + 1 == *count {
                    store.add_synced(&key, &batch).unwrap();
                    batch.clear();
                }
            }
        }
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Run `f` `runs` times and print the median and the slowest time.
fn measure<T>(label: &str, runs: usize, mut f: impl FnMut() -> T) -> T {
    let mut times = Vec::with_capacity(runs);
    let mut last = None;
    for _ in 0..runs {
        let t = Instant::now();
        last = Some(f());
        times.push(t.elapsed());
    }
    times.sort();
    println!(
        "{label:<52} median {:>8.2} ms   max {:>8.2} ms",
        ms(times[runs / 2]),
        ms(times[runs - 1])
    );
    last.unwrap()
}

fn json_kb<T: serde::Serialize>(label: &str, value: &T) {
    let kb = serde_json::to_vec(value).unwrap().len() as f64 / 1024.0;
    println!("{label:<52} {kb:>8.1} KB of JSON");
}

fn view(folder: Option<&str>, sort: SortKey, filter: &str, bodies: bool) -> ListView {
    ListView {
        folder: folder.map(str::to_owned),
        sort,
        ascending: false,
        filter: filter.to_owned(),
        bodies,
        ..ListView::default()
    }
}

/// The mail base to measure: TELEX_SCALE_DB, or a new file that is removed afterwards.
fn scale_db() -> (PathBuf, bool) {
    let keep = std::env::var_os("TELEX_SCALE_DB").map(PathBuf::from);
    let path = keep.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("telex-scale-{}.db", std::process::id()))
    });
    if !path.exists() {
        let t = Instant::now();
        let mut store = MailStore::open(&path).unwrap();
        build(&mut store);
        drop(store);
        let mb = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) as f64 / 1048576.0;
        println!(
            "built 100,000 messages in {:.1} s, {mb:.0} MB",
            t.elapsed().as_secs_f64()
        );
    }
    (path, keep.is_some())
}

fn remove_db(path: &Path) {
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[test]
#[ignore = "builds a 100,000-message mail base; run with --release -- --ignored --nocapture"]
fn hundred_thousand_messages() {
    let (path, keep) = scale_db();
    if let Some(dir) = std::env::var_os("TELEX_SCALE_EXPORT").map(PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        let export_store = MailStore::open(&path).unwrap();
        let tree = export_store.folder_tree().unwrap();
        let mut folders = Vec::new();
        for (key, counts) in &tree.counts {
            if counts.total == 0 {
                continue;
            }
            let file = format!("{}.mbox", key.replace('/', "--"));
            let exported =
                telex_core::mailbox::export_folder(&path, key, &dir.join(&file)).unwrap();
            assert_eq!(exported, counts.total);
            folders.push(serde_json::json!({"key": key, "file": file, "messages": exported}));
        }
        folders.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
        std::fs::write(
            dir.join("fixture.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": 1, "id": "telex-scale-100k-v1", "seed": "7e1ec0de20261010",
                "messages": 100_000, "accounts": 3, "folders": folders,
                "description": "Synthetic messages, plain text and HTML, about 10% with attachments."
            })).unwrap(),
        ).unwrap();
        println!("exported the shared mailbox to {}", dir.display());
    }
    let inbox = "twojastudio/inbox";
    println!();

    // Start: open the mail base, draw the folder tree, show the first page of the Inbox.
    let first = |store: &MailStore| {
        let tree = store.folder_tree().unwrap();
        let page = store
            .page(
                &view(Some(inbox), SortKey::Date, "", false),
                PageAt::First,
                100,
            )
            .unwrap();
        (tree, page)
    };
    measure("start: open, folder tree, first page", 5, || {
        let store = MailStore::open(&path).unwrap();
        first(&store);
    });
    let mut store = MailStore::open(&path).unwrap();
    let (tree, page) = measure("folder tree and first page (open store)", 11, || {
        first(&store)
    });
    assert_eq!(
        tree.counts.values().map(|c| c.total).sum::<usize>(),
        100_000
    );
    assert_eq!(page.total, 40_000);
    json_kb("folder tree", &tree);
    json_kb("a page of 100 summaries", &page);
    measure("folder tree alone", 11, || store.folder_tree().unwrap());

    // Switching folders: the first page of each.
    let folders: Vec<String> = tree.counts.keys().cloned().collect();
    let mut next = folders.iter().cycle();
    measure("folder switch: first page of a folder", 39, || {
        let folder = next.next().unwrap();
        store
            .page(
                &view(Some(folder), SortKey::Date, "", false),
                PageAt::First,
                100,
            )
            .unwrap()
    });

    // Scrolling and jumping inside the 40,000-message Inbox.
    let date = view(Some(inbox), SortKey::Date, "", false);
    let deep = store.page(&date, PageAt::Row(20_000), 100).unwrap();
    assert_eq!(deep.start, 20_000);
    let middle = deep.rows[50].id;
    measure("next page after row 20,000 (keyset)", 21, || {
        store
            .page(&date, PageAt::After(deep.rows[99].id), 100)
            .unwrap()
    });
    measure("page before row 20,000 (keyset)", 21, || {
        store
            .page(&date, PageAt::Before(deep.rows[0].id), 100)
            .unwrap()
    });
    measure("page around a message at row 20,050", 21, || {
        store.page(&date, PageAt::Around(middle), 100).unwrap()
    });
    measure("jump to row 20,000 (scroll bar)", 21, || {
        store.page(&date, PageAt::Row(20_000), 100).unwrap()
    });
    measure("jump to the end", 21, || {
        store.page(&date, PageAt::Last, 100).unwrap()
    });

    // The other orders.
    for sort in [
        SortKey::Subject,
        SortKey::From,
        SortKey::Size,
        SortKey::Flag,
    ] {
        let v = view(Some(inbox), sort, "", false);
        let page = measure(&format!("{sort:?}: first page"), 5, || {
            store.page(&v, PageAt::First, 100).unwrap()
        });
        measure(&format!("{sort:?}: next page"), 5, || {
            store
                .page(&v, PageAt::After(page.rows[99].id), 100)
                .unwrap()
        });
    }

    // The quick filter: headers as you type, then also the text. Each run first changes a
    // message (not timed), so the search is done again instead of read from the last result.
    let mut flip = false;
    for (folder, filter, bodies) in [
        (Some(inbox), "k", false),
        (Some(inbox), "kowal", false),
        (Some(inbox), "kowal", true),
        (Some(inbox), "projekt", true),
        (Some(inbox), "żółwia", true),
        (None, "kowalska", true),
        (None, "żółwia", true),
        (None, "faktura wrzesień", true),
    ] {
        let v = view(folder, SortKey::Date, filter, bodies);
        let label = format!(
            "filter \"{filter}\" in {}{}",
            folder.unwrap_or("all mail"),
            if bodies { " with text" } else { "" }
        );
        let mut times = Vec::new();
        let mut page = None;
        for _ in 0..5 {
            flip = !flip;
            store.set_flagged(middle, flip).unwrap();
            let t = Instant::now();
            page = Some(store.page(&v, PageAt::First, 100).unwrap());
            times.push(t.elapsed());
        }
        times.sort();
        let page = page.unwrap();
        println!(
            "{label:<52} median {:>8.2} ms   max {:>8.2} ms",
            ms(times[2]),
            ms(times[4])
        );
        println!("{:<52} {} matches", "", page.total);
        if let Some(last) = page.rows.last() {
            measure("  next page of the same result", 5, || {
                store.page(&v, PageAt::After(last.id), 100).unwrap()
            });
        }
    }

    // Opening a message: its text, and the raw message for the HTML view.
    let ids: Vec<MessageId> = deep.rows.iter().map(|m| m.id).collect();
    let mut next = ids.iter().cycle();
    measure("open a message (text and raw)", 41, || {
        let id = *next.next().unwrap();
        (store.get(id).unwrap(), store.raw_of(id).unwrap())
    });

    // Marking a message read, then the counts and the open page again.
    let mut flip = false;
    measure("mark read, new counts, open page again", 11, || {
        flip = !flip;
        store.set_unread(middle, flip).unwrap();
        (
            store.folder_tree().unwrap(),
            store.page(&date, PageAt::Around(middle), 100).unwrap(),
        )
    });

    for q in ["projekt", "żółwia", "kowalska", "faktura wrzesień", "wyc"] {
        let hits = measure(&format!("search \"{q}\""), 11, || store.search(q).unwrap());
        println!("{:<52} {} hits", "", hits.len());
    }

    // For comparison with the numbers from before paging.
    measure("list: all 40,000 summaries of one folder", 5, || {
        store.list(inbox).unwrap()
    });

    // Since 0.5: conversations (T), people to suggest, operators, ranges and bulk changes.
    let mut next = ids.iter().cycle();
    let thread = measure("conversation of a message (T)", 21, || {
        store.conversation(*next.next().unwrap()).unwrap()
    });
    println!("{:<52} {} messages in the last one", "", thread.len());
    let people = measure("recipients: everyone to suggest", 5, || {
        store.recipients().unwrap()
    });
    println!("{:<52} {} people", "", people.len());
    for (folder, filter) in [
        (Some(inbox), "is:unread"),
        (Some(inbox), "from:kowal is:flagged"),
        (None, "has:file larger:50k"),
        (None, "after:2026-01 subject:wycena"),
    ] {
        let v = view(folder, SortKey::Date, filter, false);
        let mut times = Vec::new();
        let mut total = 0;
        for _ in 0..5 {
            flip = !flip;
            store.set_flagged(middle, flip).unwrap();
            let t = Instant::now();
            total = store.page(&v, PageAt::First, 100).unwrap().total;
            times.push(t.elapsed());
        }
        times.sort();
        println!(
            "{:<52} median {:>8.2} ms   max {:>8.2} ms",
            format!("filter \"{filter}\" in {}", folder.unwrap_or("all mail")),
            ms(times[2]),
            ms(times[4])
        );
        println!("{:<52} {total} matches", "");
    }
    measure("Shift+click: ids from row 20,000 to 20,099", 11, || {
        store
            .ids_between(&date, Some(deep.rows[0].id), Some(deep.rows[99].id))
            .unwrap()
    });
    measure("Ctrl+A: every id of the 40,000", 5, || {
        store.ids_between(&date, None, None).unwrap()
    });
    let mut read = false;
    measure("mark 100 messages read or unread at once", 11, || {
        read = !read;
        store.set_unread_many(&ids, read).unwrap()
    });
    let archive = "twojastudio/archive";
    measure("move 100 messages to Archive and back", 5, || {
        store.move_many(&ids, archive).unwrap();
        store.move_many(&ids, inbox).unwrap()
    });

    // Since 0.9.5: thread rows (Settings › Reading). The reply graph of the account is worked
    // out once; reading and flagging mail must not work it out again.
    let threaded = ListView {
        grouped: true,
        ..date.clone()
    };
    let mut moved = false;
    let page = measure("threaded: first page after mail came or went", 5, || {
        moved = !moved;
        let to = if moved { archive } else { inbox };
        store.move_many(&ids[..1], to).unwrap();
        store.page(&threaded, PageAt::First, 100).unwrap()
    });
    if moved {
        store.move_many(&ids[..1], inbox).unwrap();
    }
    println!("{:<52} {} rows", "", page.total);
    let deep = store.page(&threaded, PageAt::Row(15_000), 100).unwrap();
    let middle = deep.rows[50].id;
    measure("threaded: next page after row 15,000", 21, || {
        store
            .page(&threaded, PageAt::After(deep.rows[99].id), 100)
            .unwrap()
    });
    measure("threaded: jump to row 15,000 (scroll bar)", 21, || {
        store.page(&threaded, PageAt::Row(15_000), 100).unwrap()
    });
    let mut flip = false;
    measure(
        "threaded: mark read, new counts, open page again",
        21,
        || {
            flip = !flip;
            store.set_unread(middle, flip).unwrap();
            (
                store.folder_tree().unwrap(),
                store.page(&threaded, PageAt::Around(middle), 100).unwrap(),
            )
        },
    );
    measure("threaded: mark read, next unread (Space)", 21, || {
        flip = !flip;
        store.set_unread(middle, flip).unwrap();
        store.next_unread(&threaded, Some(middle), false).unwrap()
    });
    measure("threaded: Shift+click across 100 rows", 21, || {
        store
            .ids_between(&threaded, Some(deep.rows[0].id), Some(deep.rows[99].id))
            .unwrap()
    });

    // A rule's preview (Rules › Preview) holds the mail base while it reads messages, so a
    // check or a send waits for it: it reads a limited number of the list's messages.
    let rule = Rule {
        id: 0,
        name: "Żółw".into(),
        enabled: true,
        account: None,
        match_all: true,
        conditions: vec![Condition {
            field: Field::Body,
            op: Op::Contains,
            value: "żółwia".into(),
            header: None,
        }],
        actions: vec![Action::Flag],
        stop: false,
    };
    let report = measure("rule preview: body contains, all mail", 3, || {
        filters::preview(&store, &rule, &view(None, SortKey::Date, "", false)).unwrap()
    });
    println!(
        "{:<52} {} of {} checked match",
        "", report.matched, report.checked
    );

    drop(store);
    if !keep {
        remove_db(&path);
    }
}

/// Upgrading a version 6 mail base (TELEX_SCALE_V6, made by this test before version 7)
/// to version 7, on a copy. Searches must find the same messages in the same order.
#[test]
#[ignore = "needs TELEX_SCALE_V6=<a version 6 mail base>"]
fn upgrading_a_hundred_thousand_messages() {
    let Some(v6) = std::env::var_os("TELEX_SCALE_V6").map(PathBuf::from) else {
        println!("Set TELEX_SCALE_V6 to a version 6 mail base.");
        return;
    };
    let copy = std::env::temp_dir().join(format!("telex-v6-{}.db", std::process::id()));
    std::fs::copy(&v6, &copy).unwrap();
    let mb = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) as f64 / 1048576.0;
    let queries = [
        "projekt",
        "żółwia",
        "kowalska",
        "faktura wrzesień",
        "wyc",
        "zolw",
    ];
    let before: Vec<Vec<i64>> = {
        let db = rusqlite::Connection::open(&copy).unwrap();
        let version: i64 = db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 6, "TELEX_SCALE_V6 must be a version 6 mail base");
        let mut stmt = db
            .prepare("SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT 5000")
            .unwrap();
        queries
            .iter()
            .map(|q| {
                // As MailStore::search words it: each word quoted, as a prefix, ł as l.
                let words: Vec<String> = q
                    .split_whitespace()
                    .map(|w| format!("\"{}\"*", w.replace('ł', "l")))
                    .collect();
                measure(&format!("version 6: search \"{q}\""), 7, || {
                    stmt.query_map([words.join(" ")], |r| r.get(0))
                        .unwrap()
                        .collect::<Result<Vec<i64>, _>>()
                        .unwrap()
                })
            })
            .collect()
    };
    println!();
    println!("{:<52} {:>8.0} MB", "version 6 file", mb(&copy));
    let t = Instant::now();
    let store = MailStore::open(&copy).unwrap();
    println!(
        "{:<52} {:>8.1} s",
        "upgrade to version 7",
        t.elapsed().as_secs_f64()
    );
    for (q, hits) in queries.iter().zip(&before) {
        let now: Vec<i64> = measure(&format!("version 7: search \"{q}\""), 7, || {
            store.search(q).unwrap()
        })
        .into_iter()
        .map(|id| id as i64)
        .collect();
        assert_eq!(&now, hits, "same hits in the same order for \"{q}\"");
    }
    println!(
        "{:<52} identical for {} queries",
        "search results",
        queries.len()
    );
    drop(store);
    println!("{:<52} {:>8.0} MB", "version 7 file", mb(&copy));
    remove_db(&copy);
}
