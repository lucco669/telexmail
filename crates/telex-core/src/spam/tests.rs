use super::*;
use crate::i18n::{self, Lang};
use crate::model::{Account, AccountSetup, Folder};
use crate::store::SyncedMessage;

/// A store with one server account: Inbox, Sent, Archive, Spam and Trash.
fn store() -> (MailStore, Account) {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Studio".into(),
            email: "biuro@twojastudio.pl".into(),
            host: "mail.twojastudio.pl".into(),
            port: 993,
            username: "biuro@twojastudio.pl".into(),
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
                folder("sent", FolderKind::Sent),
                folder("archive", FolderKind::Archive),
                folder("spam", FolderKind::Spam),
                folder("trash", FolderKind::Trash),
            ],
        )
        .unwrap();
    let account = store.account(&account.id).unwrap().clone();
    (store, account)
}

/// A tiny random generator, so the synthetic mail is the same at every run.
struct Dice(u64);

impl Dice {
    fn roll(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % n as u64) as usize
    }

    fn pick<'a>(&mut self, words: &[&'a str]) -> &'a str {
        words[self.roll(words.len())]
    }

    fn text(&mut self, own: &[&str], n: usize) -> String {
        (0..n)
            .map(|_| {
                if self.roll(3) == 0 {
                    self.pick(COMMON)
                } else {
                    self.pick(own)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

const COMMON: &[&str] = &[
    "please",
    "today",
    "information",
    "message",
    "regards",
    "thanks",
    "dzień",
    "dobry",
    "pozdrawiam",
    "jutro",
    "email",
    "here",
    "with",
    "your",
];
const HAM: &[&str] = &[
    "faktura",
    "projekt",
    "spotkanie",
    "wycena",
    "umowa",
    "termin",
    "poprawki",
    "makieta",
    "klient",
    "budżet",
    "harmonogram",
    "logo",
    "strona",
    "przegląd",
    "środa",
    "piątek",
    "załącznik",
    "prezentacja",
    "zespół",
    "wdrożenie",
];
const SPAM: &[&str] = &[
    "lottery",
    "winner",
    "prize",
    "claim",
    "bitcoin",
    "casino",
    "viagra",
    "investment",
    "guaranteed",
    "unclaimed",
    "inheritance",
    "bonus",
    "jackpot",
    "crypto",
    "pills",
    "discount",
    "urgent",
    "beneficiary",
    "wygrana",
    "nagroda",
];
const FRIENDS: &[&str] = &[
    "anna@kowalska-architekci.pl",
    "piotr@nowak-meble.pl",
    "kasia@drukarnia-lodz.pl",
    "biuro@ksiegowosc-wrona.pl",
    "marek@agencja-pion.pl",
];

fn raw(from: &str, subject: &str, body: &str, extra: &str, n: usize) -> Vec<u8> {
    format!(
        "From: {from}\r\nTo: biuro@twojastudio.pl\r\nSubject: {subject}\r\n\
         Message-ID: <{n}.{}@example.net>\r\nDate: Thu, 15 Oct 2026 10:00:00 +0200\r\n{extra}\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n",
        subject.len()
    )
    .into_bytes()
}

fn ham(dice: &mut Dice, n: usize) -> Vec<u8> {
    let from = dice.pick(FRIENDS);
    let subject = dice.text(HAM, 3);
    let body = dice.text(HAM, 40);
    raw(
        from,
        &subject,
        &body,
        "Authentication-Results: mx.twojastudio.pl; spf=pass dkim=pass\r\n",
        n,
    )
}

fn spam(dice: &mut Dice, n: usize) -> Vec<u8> {
    let from = format!("promo{}@deal{}.example", dice.roll(1000), dice.roll(50));
    let subject = format!("{}!!", dice.text(SPAM, 3).to_uppercase());
    let body = format!(
        "{} http://win-{}.example/claim?id={n}",
        dice.text(SPAM, 40),
        dice.pick(SPAM)
    );
    raw(
        &from,
        &subject,
        &body,
        "Authentication-Results: mx.twojastudio.pl; spf=softfail dkim=none\r\n",
        n,
    )
}

/// Store `raw` in the account's `folder`, read or unread.
fn add(
    store: &mut MailStore,
    account: &Account,
    folder: &str,
    raw: Vec<u8>,
    read: bool,
) -> MessageId {
    let key = account.folder_key(folder);
    let mut message = crate::mime::parse_message(&raw, &key, None);
    message.unread = !read;
    let uid = store
        .local_uids(&key)
        .unwrap()
        .into_iter()
        .max()
        .unwrap_or(0)
        + 1;
    store
        .add_synced(&key, &[SyncedMessage { uid, message, raw }])
        .unwrap()[0]
}

/// A store that has learned from `spams` spam and `hams` good messages.
fn trained(spams: usize, hams: usize) -> (MailStore, Account, Dice) {
    let (mut store, account) = store();
    let mut dice = Dice(7);
    for n in 0..spams {
        let m = spam(&mut dice, n);
        add(&mut store, &account, "spam", m, false);
    }
    for n in 0..hams {
        let m = ham(&mut dice, 1000 + n);
        add(&mut store, &account, "inbox", m, true);
    }
    while learn_pending(&mut store, LEARN_BATCH).unwrap() > 0 {}
    (store, account, dice)
}

fn folder(store: &MailStore, id: MessageId) -> String {
    store.folder_of(id).unwrap().unwrap()
}

fn counts(store: &MailStore, token: &str) -> (u32, u32) {
    store
        .db
        .query_row(
            "SELECT spam, ham FROM spam_tokens WHERE token = ?1",
            [token],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap()
        .unwrap_or((0, 0))
}

#[test]
fn chi_squared_and_fisher_combine_as_expected() {
    // With two degrees of freedom the tail is e^(-x/2).
    assert!((chi2q(2.0, 2) - (-1.0f64).exp()).abs() < 1e-12);
    assert_eq!(chi2q(0.0, 10), 1.0);
    assert_eq!(combine(&[]), 0.5);
    assert!(combine(&[0.99; 20]) > 0.99);
    assert!(combine(&[0.01; 20]) < 0.01);
    // Evidence both ways: it can't tell.
    let mixed: Vec<f64> = [0.99, 0.01].repeat(10);
    assert!((combine(&mixed) - 0.5).abs() < 0.01);
    // A token seen only in spam leans to spam, and more so the more often.
    let once = token_score(1, 0, 100, 100).unwrap();
    let often = token_score(30, 0, 100, 100).unwrap();
    assert!(0.5 < once && once < often && often < 1.0);
    assert_eq!(token_score(0, 0, 100, 100), None);
    // Seen in the same share of each, whatever their sizes: no lean.
    assert!((token_score(10, 100, 50, 500).unwrap() - 0.5).abs() < 1e-12);
}

#[test]
fn tokens_fold_words_and_read_links_senders_and_headers() {
    let raw = "From: \"Wygrana Dnia\" <Promo1234@Mail.Deals.Example>\r\n\
        To: a@b.pl, c@d.pl\r\nSubject: ZAPRASZAMY PO NAGRODĘ!!\r\n\
        Message-ID: <x@bulk.sender.example>\r\n\
        Reply-To: claims@other.example\r\n\
        Authentication-Results: mx.b.pl; spf=fail smtp.mailfrom=x; dkim=none\r\n\
        Authentication-Results: forged; spf=pass\r\n\
        List-Unsubscribe: <mailto:u@x>\r\n\
        Content-Type: text/plain; charset=UTF-8\r\n\r\n\
        Żółta łódź czeka: https://user@claim.prize.example:8080/now i www.second.example 12345\r\n";
    let m = crate::mime::parse_message(raw.as_bytes(), "a/inbox", None);
    let t = tokens(&m, Some(raw.as_bytes()), &HashSet::new());
    for want in [
        "subject:nagrode",
        "subject:zapraszamy",
        "subject:!!",
        "subject:shouting",
        "zolta",
        "lodz",
        "czeka",
        "from:promo1234@mail.deals.example",
        "from:@mail.deals.example",
        "from:@@deals.example",
        "from:digits",
        "name:wygrana",
        "to:few",
        "url:claim.prize.example",
        "url:@@prize.example",
        "url:www.second.example",
        "auth:spf=fail",
        "auth:dkim=none",
        "header:list-unsubscribe",
        "reply-to:elsewhere",
        "msgid:@@sender.example",
        "charset:utf-8",
        "type:text/plain",
    ] {
        assert!(t.contains(want), "{want} missing from {t:?}");
    }
    // Only the first Authentication-Results counts; numbers alone are no words.
    assert!(!t.contains("auth:spf=pass"));
    assert!(!t.contains("12345"));

    // The user's own address says nothing about a message.
    let own = HashSet::from(["promo1234@mail.deals.example".to_owned()]);
    let t = tokens(&m, None, &own);
    assert!(!t
        .iter()
        .any(|t| t.starts_with("from:") || t.starts_with("name:")));
}

#[test]
fn learned_mail_sorts_new_mail_and_says_why() {
    let (mut store, account, mut dice) = trained(60, 60);
    assert_eq!(
        store.spam_status().unwrap(),
        Status {
            spam: 60,
            ham: 60,
            needed: MIN_LEARNED
        }
    );
    let junk: Vec<MessageId> = (0..10)
        .map(|n| {
            let m = spam(&mut dice, 5000 + n);
            add(&mut store, &account, "inbox", m, false)
        })
        .collect();
    let good: Vec<MessageId> = (0..10)
        .map(|n| {
            let m = ham(&mut dice, 6000 + n);
            add(&mut store, &account, "inbox", m, false)
        })
        .collect();
    let all: Vec<MessageId> = junk.iter().chain(&good).copied().collect();
    let moved = i18n::with(Lang::En, || {
        sort_arrivals(&mut store, &all, Level::Careful).unwrap()
    });
    assert_eq!(moved, junk, "every spam message and nothing else");
    for &id in &junk {
        assert_eq!(folder(&store, id), account.folder_key("spam"));
        let note = store.get(id).unwrap().note.unwrap();
        assert!(
            note.starts_with("The spam filter moved it to Spam"),
            "{note}"
        );
        assert!(note.contains("telling words:"), "{note}");
        assert!(store.spam_verdict(id).unwrap().score > 0.99);
    }
    for &id in &good {
        assert_eq!(folder(&store, id), account.folder_key("inbox"));
        assert!(store.get(id).unwrap().note.is_none());
        assert!(store.spam_verdict(id).unwrap().score < 0.2);
    }
    // The server hears of the moves like any other.
    let queued = store.pending_changes(&account.id).unwrap();
    assert_eq!(queued.len(), junk.len());
    // What the filter moved itself teaches it nothing, now or later.
    assert_eq!(store.spam_status().unwrap().spam, 60);
    learn_pending(&mut store, LEARN_BATCH).unwrap();
    assert_eq!(store.spam_status().unwrap().spam, 60);
}

#[test]
fn the_note_speaks_polish() {
    let verdict = Verdict {
        score: 0.999,
        clues: vec![
            ("subject:nagroda".into(), 0.99),
            ("auth:spf=fail".into(), 0.98),
            ("url:@@prize.example".into(), 0.97),
            ("projekt".into(), 0.01),
            ("wygrana".into(), 0.96),
        ],
    };
    assert_eq!(
        i18n::with(Lang::Pl, || note(&verdict)),
        "Filtr spamu przeniósł ją do Spamu (ocena 99 na 100; słowa, które przesądziły: \
         nagroda, prize.example, wygrana). Jeśli to nie spam, wybierz „To nie spam” z jej menu."
    );
    let bare = Verdict {
        score: 0.95,
        clues: vec![],
    };
    assert!(i18n::with(Lang::En, || note(&bare)).contains("(spam score 95 of 100)"));
}

#[test]
fn nothing_moves_until_enough_was_learned_or_when_off() {
    let (mut store, account, mut dice) = trained(10, 80);
    let m = spam(&mut dice, 7000);
    let id = add(&mut store, &account, "inbox", m, false);
    assert!(sort_arrivals(&mut store, &[id], Level::Strict)
        .unwrap()
        .is_empty());

    let (mut store, account, mut dice) = trained(60, 60);
    let m = spam(&mut dice, 7000);
    let id = add(&mut store, &account, "inbox", m, false);
    assert!(sort_arrivals(&mut store, &[id], Level::Off)
        .unwrap()
        .is_empty());
    assert_eq!(folder(&store, id), account.folder_key("inbox"));
}

#[test]
fn people_written_to_trusted_senders_and_answers_are_left_alone() {
    let (mut store, account, mut dice) = trained(60, 60);
    // The user wrote to one spammer once, and trusts another.
    let pen_pal = spam(&mut dice, 8000);
    let pen_pal_id = add(&mut store, &account, "inbox", pen_pal, false);
    let pen_pal_from = store.get(pen_pal_id).unwrap().from_email;
    let sent = raw(
        "biuro@twojastudio.pl",
        "Re: oferta",
        "Nie, dziękuję.",
        "",
        8001,
    );
    let sent = String::from_utf8(sent)
        .unwrap()
        .replace("To: biuro@twojastudio.pl", &format!("To: {pen_pal_from}"));
    add(&mut store, &account, "sent", sent.into_bytes(), true);

    let trusted = spam(&mut dice, 8002);
    let trusted_id = add(&mut store, &account, "inbox", trusted, false);
    let trusted_from = store.get(trusted_id).unwrap().from_email;
    store.set_sender_trusted(&trusted_from, true).unwrap();

    // An answer to the user's own message (its Message-ID: <8001.10@example.net>).
    let answer = String::from_utf8(spam(&mut dice, 8003)).unwrap().replace(
        "Content-Type:",
        "In-Reply-To: <8001.10@example.net>\r\nContent-Type:",
    );
    let answer_id = add(&mut store, &account, "inbox", answer.into_bytes(), false);

    let stranger = spam(&mut dice, 8004);
    let stranger_id = add(&mut store, &account, "inbox", stranger, false);

    let ids = [pen_pal_id, trusted_id, answer_id, stranger_id];
    let moved = sort_arrivals(&mut store, &ids, Level::Strict).unwrap();
    assert_eq!(moved, [stranger_id]);
}

fn bank_scam(n: usize) -> Vec<u8> {
    raw(
        "iPKObiznes <notice@account-update.example>",
        "Potwierdz konto",
        "Zaloguj sie: https://account-check.example/login",
        "",
        n,
    )
}

#[test]
fn high_scam_risk_is_sorted_without_training_and_not_self_learned() {
    for level in [Level::Careful, Level::Normal, Level::Strict] {
        let (mut store, account) = store();
        let id = add(&mut store, &account, "inbox", bank_scam(8100), false);
        assert_eq!(store.inspect_message(id).unwrap().risk.score, 75);
        assert_eq!(sort_arrivals(&mut store, &[id], level).unwrap(), [id]);
        assert_eq!(folder(&store, id), account.folder_key("spam"));
        assert!(store.get(id).unwrap().note.unwrap().contains("scam risk"));
        assert_eq!(class_of(&store.db, id).unwrap(), Some(Class::Judged));
        assert_eq!(learn_pending(&mut store, LEARN_BATCH).unwrap(), 0);
        assert_eq!(totals(&store.db).unwrap(), (0, 0));

        store.mark_spam(&[id], false).unwrap();
        assert!(sort_arrivals(&mut store, &[id], level).unwrap().is_empty());
        assert_eq!(folder(&store, id), account.folder_key("inbox"));
    }
}

#[test]
fn scam_sorting_respects_off_trusted_senders_and_lower_risk() {
    let (mut store, account) = store();
    let id = add(&mut store, &account, "inbox", bank_scam(8101), false);
    assert!(sort_arrivals(&mut store, &[id], Level::Off)
        .unwrap()
        .is_empty());
    store
        .set_sender_trusted("notice@account-update.example", true)
        .unwrap();
    assert!(sort_arrivals(&mut store, &[id], Level::Careful)
        .unwrap()
        .is_empty());
    store
        .set_sender_trusted("notice@account-update.example", false)
        .unwrap();
    let mild = raw(
        "PKO <news@unrelated.example>",
        "News",
        "Our newsletter",
        "",
        8102,
    );
    let mild = add(&mut store, &account, "inbox", mild, false);
    let score = store.inspect_message(mild).unwrap().risk.score;
    assert!(score > 0 && score < 70);
    assert!(sort_arrivals(&mut store, &[mild], Level::Strict)
        .unwrap()
        .is_empty());
    assert_eq!(
        sort_arrivals(&mut store, &[id], Level::Careful).unwrap(),
        [id]
    );
}

#[test]
fn genuine_ipko_biznes_mail_stays_in_the_inbox() {
    let (mut store, account) = store();
    let mail = raw(
        "iPKObiznes <info@pkobp.pl>",
        "Potwierdz konto",
        "Zaloguj sie: https://www.ipkobiznes.pl/",
        "",
        8103,
    );
    let id = add(&mut store, &account, "inbox", mail, false);
    assert_eq!(store.inspect_message(id).unwrap().risk.score, 0);
    assert!(sort_arrivals(&mut store, &[id], Level::Strict)
        .unwrap()
        .is_empty());
}

#[test]
fn html_only_credential_lure_is_filtered_from_its_actual_destination() {
    let (mut store, account) = store();
    let mail = raw(
        "PKO Bank Polski <info@pkobp.pl>",
        "Wiadomosc",
        "<a href='https://account-check.example/login'>Zaloguj sie</a>",
        "",
        8104,
    );
    let mail = String::from_utf8(mail)
        .unwrap()
        .replace("text/plain", "text/html")
        .into_bytes();
    let id = add(&mut store, &account, "inbox", mail, false);
    assert_eq!(
        sort_arrivals(&mut store, &[id], Level::Careful).unwrap(),
        [id]
    );
}

#[test]
fn moving_into_spam_and_back_learns_again() {
    let (mut store, account, mut dice) = trained(60, 60);
    let m = ham(&mut dice, 9000).into_iter().collect::<Vec<u8>>();
    let m = String::from_utf8(m)
        .unwrap()
        .replace("Content-Type:", "X-Mailer: Kwiatuszek 3\r\nContent-Type:");
    let id = add(&mut store, &account, "inbox", m.into_bytes(), true);
    learn_pending(&mut store, LEARN_BATCH).unwrap();
    assert_eq!(counts(&store, "mailer:kwiatuszek"), (0, 1));
    assert_eq!(counts(&store, ""), (60, 61));

    // Spam, from the menu: learned as spam, no longer as good mail.
    let places = store.mark_spam(&[id], true).unwrap();
    assert_eq!(places, [(id, account.folder_key("inbox"))]);
    assert_eq!(folder(&store, id), account.folder_key("spam"));
    assert_eq!(counts(&store, "mailer:kwiatuszek"), (1, 0));
    assert_eq!(counts(&store, ""), (61, 60));

    // Undo is a move back: good mail again. Moving it about further teaches nothing new.
    store
        .move_many(&[id], &account.folder_key("inbox"))
        .unwrap();
    assert_eq!(counts(&store, "mailer:kwiatuszek"), (0, 1));
    assert_eq!(counts(&store, ""), (60, 61));
    store
        .move_many(&[id], &account.folder_key("archive"))
        .unwrap();
    store.delete(id).unwrap();
    assert_eq!(counts(&store, ""), (60, 61));
    // From Trash into Spam and back out: each move learns once.
    store.mark_spam(&[id], true).unwrap();
    store.mark_spam(&[id], false).unwrap();
    assert_eq!(folder(&store, id), account.folder_key("inbox"));
    assert_eq!(counts(&store, "mailer:kwiatuszek"), (0, 1));
}

#[test]
fn not_spam_teaches_the_filter_its_mistake() {
    let (mut store, account, mut dice) = trained(60, 60);
    let m = spam(&mut dice, 9100);
    let id = add(&mut store, &account, "inbox", m, false);
    assert_eq!(
        sort_arrivals(&mut store, &[id], Level::Careful).unwrap(),
        [id]
    );
    assert_eq!(counts(&store, ""), (60, 60));
    let places = store.mark_spam(&[id], false).unwrap();
    assert_eq!(places, [(id, account.folder_key("spam"))]);
    assert_eq!(folder(&store, id), account.folder_key("inbox"));
    // Learned as good mail; it had never been learned as spam.
    assert_eq!(counts(&store, ""), (60, 61));
}

#[test]
fn unread_mail_waits_and_mail_on_this_pc_has_no_spam_folder() {
    let (mut store, account, mut dice) = trained(0, 0);
    let m = ham(&mut dice, 1);
    let unread = add(&mut store, &account, "inbox", m, false);
    let m = ham(&mut dice, 2);
    add(&mut store, &account, "trash", m, true);
    assert_eq!(learn_pending(&mut store, LEARN_BATCH).unwrap(), 0);
    store.set_unread(unread, false).unwrap();
    assert_eq!(learn_pending(&mut store, LEARN_BATCH).unwrap(), 1);
    assert_eq!(learn_pending(&mut store, LEARN_BATCH).unwrap(), 0);

    let local = store.create_local_folder("Stare").unwrap();
    store.move_many(&[unread], &local).unwrap();
    let error = i18n::with(Lang::En, || store.mark_spam(&[unread], true).unwrap_err());
    assert!(error.to_string().contains("no Spam folder"), "{error}");
    assert_eq!(folder(&store, unread), local);
}

#[test]
fn levels_have_rising_cutoffs() {
    assert_eq!(Level::Off.cutoff(), None);
    let cut = |l: Level| l.cutoff().unwrap();
    assert!(cut(Level::Careful) > cut(Level::Normal) && cut(Level::Normal) > cut(Level::Strict));
    assert_eq!(Level::default(), Level::Careful);
    assert_eq!(serde_json::to_value(Level::Careful).unwrap(), "careful");
}
