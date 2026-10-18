use super::invites::{calendar_of, Blocked, State};
use super::*;
use crate::i18n::{with, Lang};
use crate::model::{AccountSetup, Folder, FolderKind, MessageId, SmtpSecurity};
use crate::store::{MailStore, SyncedMessage};
use base64::Engine as _;
use chrono::{Local, NaiveDate};
use chrono_tz::{America, Europe};
use std::time::Duration;

fn crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, mo, d)
        .unwrap()
        .and_hms_opt(h, mi, 0)
        .unwrap()
}

fn b64(bytes: &[u8]) -> String {
    let all = base64::engine::general_purpose::STANDARD.encode(bytes);
    all.as_bytes()
        .chunks(76)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Quoted-printable, as Apple Mail writes it.
fn qp(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut width = 0;
    for &b in bytes {
        if b == b'\n' {
            out.push_str("\r\n");
            width = 0;
            continue;
        }
        if b == b'\r' {
            continue;
        }
        let piece = if (33..=126).contains(&b) && b != b'=' || b == b' ' {
            (b as char).to_string()
        } else {
            format!("={b:02X}")
        };
        if width + piece.len() > 73 {
            out.push_str("=\r\n");
            width = 0;
        }
        width += piece.len();
        out.push_str(&piece);
    }
    out
}

// ---- Samples, written after what each program sends ----

const OUTLOOK_ICS: &str = "BEGIN:VCALENDAR
METHOD:REQUEST
PRODID:Microsoft Exchange Server 2010
VERSION:2.0
BEGIN:VTIMEZONE
TZID:Central European Standard Time
BEGIN:STANDARD
DTSTART:16010101T030000
TZOFFSETFROM:+0200
TZOFFSETTO:+0100
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=10
END:STANDARD
BEGIN:DAYLIGHT
DTSTART:16010101T020000
TZOFFSETFROM:+0100
TZOFFSETTO:+0200
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=3
END:DAYLIGHT
END:VTIMEZONE
BEGIN:VEVENT
ORGANIZER;CN=\"Kowalska, Anna\":mailto:anna@firma.pl
ATTENDEE;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE;CN=Biuro Stud
 io:mailto:biuro@twojastudio.pl
ATTENDEE;ROLE=OPT-PARTICIPANT;PARTSTAT=TENTATIVE;RSVP=TRUE;CN=Piotr Nowak:mai
 lto:piotr@nowak-meble.pl
ATTENDEE;CUTYPE=RESOURCE;ROLE=NON-PARTICIPANT;RSVP=TRUE;CN=Sala 2:mailto:sal
 a2@firma.pl
DESCRIPTION;LANGUAGE=pl-PL:Omówimy makiety i harmonogram.\\n\\n______________
 __\\nSpotkanie w aplikacji Microsoft Teams\\nDołącz teraz<https://teams.micr
 osoft.com/l/meetup-join/19%3ameeting_NjA%40thread.v2/0?context=%7b%22Tid%22
 %3a%22abc%22%7d>\\n
UID:040000008200E00074C5B7101A82E0080000000010B1F5B6D24DDB01000000000000000
 010000000
SUMMARY;LANGUAGE=pl-PL:Przegląd projektu
DTSTART;TZID=Central European Standard Time:20261020T100000
DTEND;TZID=Central European Standard Time:20261020T110000
CLASS:PUBLIC
PRIORITY:5
DTSTAMP:20261016T081500Z
TRANSP:OPAQUE
STATUS:CONFIRMED
SEQUENCE:0
LOCATION;LANGUAGE=pl-PL:Sala 2\\; Microsoft Teams Meeting
X-MICROSOFT-CDO-APPT-SEQUENCE:0
X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE
X-MICROSOFT-SKYPETEAMSMEETINGURL:https://teams.microsoft.com/l/meetup-join/
 19%3ameeting_NjA%40thread.v2/0?context=%7b%22Tid%22%3a%22abc%22%7d
BEGIN:VALARM
DESCRIPTION:REMINDER
TRIGGER;RELATED=START:-PT15M
ACTION:DISPLAY
END:VALARM
END:VEVENT
END:VCALENDAR
";

/// Outlook: multipart/alternative, the calendar a nameless base64 text/calendar part.
fn outlook_mail(ics: &str, id: &str) -> Vec<u8> {
    format!(
        "From: \"Kowalska, Anna\" <anna@firma.pl>\r\n\
To: Biuro Studio <biuro@twojastudio.pl>\r\n\
Subject: =?utf-8?Q?Przegl=C4=85d_projektu?=\r\n\
Date: Fri, 16 Oct 2026 10:15:00 +0200\r\n\
Message-ID: <{id}@firma.pl>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"_000_\"\r\n\
\r\n\
--_000_\r\n\
Content-Type: text/plain; charset=\"utf-8\"\r\n\
Content-Transfer-Encoding: 8bit\r\n\
\r\n\
Omówimy makiety.\r\n\
--_000_\r\n\
Content-Type: text/calendar; charset=\"utf-8\"; method=REQUEST\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
{}\r\n\
--_000_--\r\n",
        b64(crlf(ics).as_bytes())
    )
    .into_bytes()
}

const GOOGLE_ICS: &str = "BEGIN:VCALENDAR
PRODID:-//Google Inc//Google Calendar 70.9054//EN
VERSION:2.0
CALSCALE:GREGORIAN
METHOD:REQUEST
BEGIN:VTIMEZONE
TZID:America/New_York
X-LIC-LOCATION:America/New_York
BEGIN:DAYLIGHT
TZOFFSETFROM:-0500
TZOFFSETTO:-0400
TZNAME:EDT
DTSTART:19700308T020000
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU
END:DAYLIGHT
BEGIN:STANDARD
TZOFFSETFROM:-0400
TZOFFSETTO:-0500
TZNAME:EST
DTSTART:19701101T020000
RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU
END:STANDARD
END:VTIMEZONE
BEGIN:VEVENT
DTSTART;TZID=America/New_York:20261103T090000
DTEND;TZID=America/New_York:20261103T100000
RRULE:FREQ=WEEKLY;BYDAY=TU
DTSTAMP:20261017T120000Z
ORGANIZER;CN=Jan Wiśniewski:mailto:jan@example.com
UID:7kukuqrfedlm2f9t2vr42q2d1n@google.com
ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;CN=Jan Wi
 śniewski;X-NUM-GUESTS=0:mailto:jan@example.com
ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=
 TRUE;CN=biuro@twojastudio.pl;X-NUM-GUESTS=0:mailto:biuro@twojastudio.pl
X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij
CREATED:20261017T115900Z
DESCRIPTION:Omówimy wycen{FOLD}.\\n\\nJoin with Google Meet: https://meet.go
 ogle.com/abc-defg-hij\\n\\nLearn more about Meet at: https://support.google.
 com/a/users/answer/9282720
LAST-MODIFIED:20261017T120000Z
LOCATION:
SEQUENCE:0
STATUS:CONFIRMED
SUMMARY:Rozmowa z klientem
TRANSP:OPAQUE
END:VEVENT
END:VCALENDAR
";

/// Google folds by bytes: here inside the "ę" of "wycenę" (C4 99).
fn google_ics() -> Vec<u8> {
    let text = crlf(GOOGLE_ICS);
    let (before, after) = text.split_once("{FOLD}").unwrap();
    let mut bytes = before.as_bytes().to_vec();
    bytes.extend_from_slice(b"\xc4\r\n \x99");
    bytes.extend_from_slice(after.as_bytes());
    bytes
}

/// Google: the calendar inline in the alternative part (8 bit), and again as invite.ics.
fn google_mail() -> Vec<u8> {
    let ics = google_ics();
    let mut raw = b"From: Jan Wisniewski <jan@example.com>\r\n\
To: biuro@twojastudio.pl\r\n\
Subject: Invitation: Rozmowa z klientem @ Tue Nov 3, 2026 9am - 10am (EST)\r\n\
Date: Sat, 17 Oct 2026 12:00:00 +0000\r\n\
Message-ID: <google-1@google.com>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"m\"\r\n\
\r\n\
--m\r\n\
Content-Type: multipart/alternative; boundary=\"a\"\r\n\
\r\n\
--a\r\n\
Content-Type: text/plain; charset=\"UTF-8\"\r\n\
\r\n\
Rozmowa z klientem\r\n\
--a\r\n\
Content-Type: text/html; charset=\"UTF-8\"\r\n\
\r\n\
<p>Rozmowa z klientem</p>\r\n\
--a\r\n\
Content-Type: text/calendar; charset=\"UTF-8\"; method=REQUEST\r\n\
Content-Transfer-Encoding: 7bit\r\n\
\r\n"
        .to_vec();
    raw.extend_from_slice(&ics);
    raw.extend_from_slice(
        b"\r\n--a--\r\n--m\r\n\
Content-Type: application/ics; name=\"invite.ics\"\r\n\
Content-Disposition: attachment; filename=\"invite.ics\"\r\n\
Content-Transfer-Encoding: base64\r\n\r\n",
    );
    raw.extend_from_slice(b64(&ics).as_bytes());
    raw.extend_from_slice(b"\r\n--m--\r\n");
    raw
}

const APPLE_ICS: &str = "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Apple Inc.//macOS 15.0//EN
METHOD:REQUEST
CALSCALE:GREGORIAN
BEGIN:VEVENT
UID:6B29FC40-CA47-1067-B31D-00DD010662DA
DTSTART;VALUE=DATE:20261224
DTEND;VALUE=DATE:20261227
DTSTAMP:20261017T090000Z
SEQUENCE:0
SUMMARY:Święta u rodziców
LOCATION:Kraków\\, ul. Długa 5
RRULE:FREQ=YEARLY
ORGANIZER;CN=\"Ewa Nowak\";EMAIL=\"ewa@icloud.com\":mailto:ewa@icloud.com
ATTENDEE;CN=\"Ewa Nowak\";CUTYPE=INDIVIDUAL;EMAIL=\"ewa@icloud.com\";PARTSTAT=ACCE
 PTED;ROLE=CHAIR:mailto:ewa@icloud.com
ATTENDEE;CN=\"Biuro\";CUTYPE=INDIVIDUAL;EMAIL=\"biuro@twojastudio.pl\";PARTSTAT=
 NEEDS-ACTION;ROLE=REQ-PARTICIPANT;RSVP=TRUE:mailto:biuro@twojastudio.pl
TRANSP:TRANSPARENT
URL;VALUE=URI:
END:VEVENT
END:VCALENDAR
";

/// Apple Mail: the calendar as an attached invite.ics, quoted-printable.
fn apple_mail() -> Vec<u8> {
    format!(
        "From: Ewa Nowak <ewa@icloud.com>\r\n\
To: Biuro <biuro@twojastudio.pl>\r\n\
Subject: Zaproszenie: =?utf-8?Q?=C5=9Awi=C4=99ta?=\r\n\
Date: Sat, 17 Oct 2026 11:00:00 +0200\r\n\
Message-ID: <apple-1@icloud.com>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"Apple-Mail=_1\"\r\n\
\r\n\
--Apple-Mail=_1\r\n\
Content-Type: text/plain; charset=us-ascii\r\n\
\r\n\
Zapraszam.\r\n\
--Apple-Mail=_1\r\n\
Content-Type: text/calendar; method=REQUEST; name=\"invite.ics\"; charset=utf-8\r\n\
Content-Disposition: attachment; filename=\"invite.ics\"\r\n\
Content-Transfer-Encoding: quoted-printable\r\n\
\r\n\
{}\r\n\
--Apple-Mail=_1--\r\n",
        qp(APPLE_ICS.as_bytes())
    )
    .into_bytes()
}

/// A zone Telex knows only from its VTIMEZONE: Warsaw's rules under Outlook's own name.
const CUSTOM_ZONE: &str = "BEGIN:VCALENDAR
METHOD:REQUEST
BEGIN:VTIMEZONE
TZID:Customized Time Zone
BEGIN:STANDARD
DTSTART:16010101T030000
TZOFFSETFROM:+0200
TZOFFSETTO:+0100
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=10
END:STANDARD
BEGIN:DAYLIGHT
DTSTART:16010101T020000
TZOFFSETFROM:+0100
TZOFFSETTO:+0200
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=-1SU;BYMONTH=3
END:DAYLIGHT
END:VTIMEZONE
BEGIN:VEVENT
UID:custom-1
DTSTART;TZID=Customized Time Zone:20260329T023000
DTEND;TZID=Customized Time Zone:20260329T033000
SUMMARY:Gap
END:VEVENT
END:VCALENDAR
";

// ---- Parsing ----

#[test]
fn outlook_invitation_in_local_time_and_elsewhere() {
    let raw = outlook_mail(OUTLOOK_ICS, "o1");
    let (file, cal) = calendar_of(&raw).expect("a calendar");
    assert_eq!(
        file, 0,
        "the nameless text/calendar part is the only attachment"
    );
    assert_eq!(
        crate::mime::parse_message(&raw, "x/inbox", None).attachments[0].name,
        "invite.ics"
    );
    assert_eq!(cal.method, Method::Request);
    let e = cal.main().unwrap();
    assert_eq!(
        e.uid,
        "040000008200E00074C5B7101A82E0080000000010B1F5B6D24DDB01000000000000000010000000"
    );
    assert_eq!(e.summary, "Przegląd projektu");
    assert_eq!(e.location, "Sala 2; Microsoft Teams Meeting");
    assert!(e
        .description
        .starts_with("Omówimy makiety i harmonogram.\n\n"));
    assert_eq!(
        e.organizer,
        Some(Person {
            name: "Kowalska, Anna".into(),
            email: "anna@firma.pl".into()
        })
    );
    assert_eq!(e.attendees.len(), 3);
    assert_eq!(e.attendees[0].person.email, "biuro@twojastudio.pl");
    assert_eq!(e.attendees[0].person.name, "Biuro Studio");
    assert_eq!(e.attendees[1].answer, Answer::Tentative);
    assert_eq!(e.attendees[1].role, Role::Optional);
    assert!(e.attendees[2].room);
    assert_eq!(
        e.links,
        [Link {
            label: "Microsoft Teams".into(),
            url: "https://teams.microsoft.com/l/meetup-join/19%3ameeting_NjA%40thread.v2/0?context=%7b%22Tid%22%3a%22abc%22%7d".into()
        }],
        "the same link from the property and the description, once"
    );

    let here = view(&cal, e, &Europe::Warsaw);
    assert_eq!(here.start, Some(at(2026, 10, 20, 10, 0)));
    assert_eq!(here.end, Some(at(2026, 10, 20, 11, 0)));
    assert!(!here.all_day);
    assert_eq!(here.zone, None, "written in this PC's zone");

    let away = view(&cal, e, &America::New_York);
    assert_eq!(away.start, Some(at(2026, 10, 20, 4, 0)));
    let zone = away.zone.unwrap();
    assert_eq!(zone.name, "Central European Standard Time");
    assert_eq!(zone.offset, 120);
    assert_eq!(zone.start, at(2026, 10, 20, 10, 0));
    assert_eq!(zone.end, Some(at(2026, 10, 20, 11, 0)));
}

#[test]
fn mail_that_names_a_calendar_is_found_by_its_bytes() {
    use super::invites::mentions_calendar;
    for raw in [
        &b"Content-Type: TEXT/Calendar; method=REQUEST"[..],
        b"Content-Type: application/ICS",
        b"filename=\"invite.ics\"",
        b".ics",
        b"text/calendar",
    ] {
        assert!(mentions_calendar(raw), "{}", String::from_utf8_lossy(raw));
    }
    for raw in [
        &b"Content-Type: text/plain"[..],
        b"see http://example.com/ics and a.ic",
        b"context/calenda",
        b"/calendar",
        b"text/",
        b".",
        b"",
        b"AbC/dEf+ghi/jklm.nop",
    ] {
        assert!(!mentions_calendar(raw), "{}", String::from_utf8_lossy(raw));
    }
}

#[test]
fn google_invitation_folded_inside_a_character() {
    let raw = google_mail();
    let (file, cal) = calendar_of(&raw).expect("a calendar");
    assert_eq!(file, 0, "the inline calendar before the invite.ics copy");
    let e = cal.main().unwrap();
    assert!(
        e.description.starts_with("Omówimy wycenę.\n\nJoin"),
        "{}",
        e.description
    );
    assert_eq!(e.attendees[0].person.name, "Jan Wiśniewski");
    assert_eq!(e.attendees[1].person.name, "biuro@twojastudio.pl");
    assert_eq!(
        e.links,
        [Link {
            label: "Google Meet".into(),
            url: "https://meet.google.com/abc-defg-hij".into()
        }]
    );
    // 3 November: New York has left summer time (1 Nov), Warsaw too (25 Oct).
    let v = with(Lang::En, || view(&cal, e, &Europe::Warsaw));
    assert_eq!(v.start, Some(at(2026, 11, 3, 15, 0)));
    assert_eq!(v.end, Some(at(2026, 11, 3, 16, 0)));
    let zone = v.zone.unwrap();
    assert_eq!(
        (zone.name.as_str(), zone.offset, zone.start),
        ("America/New York", -300, at(2026, 11, 3, 9, 0))
    );
    assert_eq!(v.repeats.as_deref(), Some("Every week on Tuesday"));
    let pl = with(Lang::Pl, || view(&cal, e, &Europe::Warsaw));
    assert_eq!(pl.repeats.as_deref(), Some("Co tydzień we wtorek"));
}

#[test]
fn apple_all_day_invitation_quoted_printable() {
    let raw = apple_mail();
    let (file, cal) = calendar_of(&raw).expect("a calendar");
    assert_eq!(file, 0);
    let e = cal.main().unwrap();
    assert_eq!(e.summary, "Święta u rodziców");
    assert_eq!(e.location, "Kraków, ul. Długa 5");
    assert_eq!(e.organizer.as_ref().unwrap().name, "Ewa Nowak");
    assert_eq!(e.attendees[0].role, Role::Chair);
    assert_eq!(e.attendees[1].person.email, "biuro@twojastudio.pl");
    assert!(e.links.is_empty(), "an empty URL is no link");
    let v = with(Lang::Pl, || view(&cal, e, &America::New_York));
    assert!(v.all_day);
    assert_eq!(v.start, Some(at(2026, 12, 24, 0, 0)));
    assert_eq!(
        v.end,
        Some(at(2026, 12, 26, 0, 0)),
        "DTEND is the day after"
    );
    assert_eq!(v.zone, None, "all-day dates are the same days everywhere");
    assert_eq!(v.repeats.as_deref(), Some("Co roku"));
}

#[test]
fn old_charsets_and_unicode() {
    // A calendar in Windows-1250 that says so, and one that doesn't (the Polish default).
    let ics = b"BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1\r\nSUMMARY:Spotkanie zarz\xb9du\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    for charset in [Some("windows-1250"), None] {
        let cal = parse(&decode(ics, charset)).unwrap();
        assert_eq!(cal.events[0].summary, "Spotkanie zarządu");
    }
    let raw = b"Subject: x\r\nMIME-Version: 1.0\r\nContent-Type: text/calendar; charset=iso-8859-2; method=PUBLISH\r\n\
Content-Transfer-Encoding: 8bit\r\n\r\nBEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:2\r\nSUMMARY:\xa3\xf3d\xbc\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let (_, cal) = calendar_of(raw).expect("a calendar that is the whole message");
    assert_eq!(cal.events[0].summary, "Łódź");
    assert_eq!(cal.method, Method::Publish);
}

#[test]
fn content_lines() {
    let p = content_line("ATTENDEE;CN=\"Nowak; Piotr\";DELEGATED-TO=\"mailto:a@x.pl\",\"mailto:b@x.pl\";X:mailto:p@x.pl").unwrap();
    assert_eq!(p.name, "ATTENDEE");
    assert_eq!(p.param("CN"), Some("Nowak; Piotr"));
    assert_eq!(p.param("DELEGATED-TO"), Some("mailto:a@x.pl,mailto:b@x.pl"));
    assert_eq!(p.value, "mailto:p@x.pl");
    assert_eq!(
        content_line("ORGANIZER;CN=^'Anna^' ^^1:mailto:a@x.pl")
            .unwrap()
            .param("CN"),
        Some("\"Anna\" ^1"),
        "RFC 6868 carets"
    );
    assert_eq!(content_line("no colon here"), None);
    assert_eq!(content_line("X;CN=\"unterminated:mailto:a@x.pl"), None);
    assert_eq!(text("a\\, b\\; c\\\\d\\ne\\N"), "a, b; c\\d\ne\n");
    assert_eq!(duration("PT1H30M"), Some(TimeDelta::minutes(90)));
    assert_eq!(duration("-P1W"), Some(TimeDelta::weeks(-1)));
    assert_eq!(duration("P1DT2H"), Some(TimeDelta::hours(26)));
    assert_eq!(duration("P99999999999999999999D"), None);
    assert_eq!(duration("PT"), Some(TimeDelta::zero()));
    assert_eq!(duration("1H"), None);
}

// ---- Time zones ----

#[test]
fn names_iana_windows_and_paths() {
    assert_eq!(zones::iana("Europe/Warsaw"), Some(Europe::Warsaw));
    assert_eq!(zones::iana("\"Europe/Warsaw\""), Some(Europe::Warsaw));
    assert_eq!(
        zones::iana("/mozilla.org/20050126_1/America/New_York"),
        Some(America::New_York)
    );
    assert_eq!(
        zones::iana("/freeassociation.sourceforge.net/Tzfile/America/Argentina/Buenos_Aires"),
        Some(America::Argentina::Buenos_Aires)
    );
    assert_eq!(
        zones::iana("central european standard time"),
        Some(Europe::Warsaw)
    );
    assert_eq!(
        zones::iana("Pacific Standard Time"),
        Some(America::Los_Angeles)
    );
    assert_eq!(zones::iana("Customized Time Zone"), None);
    for (windows, iana) in zones::windows_names() {
        assert!(iana.parse::<chrono_tz::Tz>().is_ok(), "{windows} → {iana}");
    }
    // Summer in Los Angeles is UTC−7 though Windows calls it "Standard".
    let pacific = zones::find("Pacific Standard Time", &[]).unwrap();
    assert_eq!(pacific.offset_at(at(2026, 7, 1, 12, 0)), -7 * 3600);
    assert_eq!(pacific.offset_at(at(2026, 12, 1, 12, 0)), -8 * 3600);
}

#[test]
fn summer_time_edges_by_name_and_by_vtimezone() {
    let cal = parse(&crlf(CUSTOM_ZONE)).unwrap();
    let rules = zones::find("Customized Time Zone", &cal.zones).expect("from the VTIMEZONE");
    assert_eq!(rules.name(), "Customized Time Zone");
    let iana = zones::find("Europe/Warsaw", &[]).unwrap();
    for zone in [&rules, &iana] {
        // 29 March 2026, 02:00 → 03:00: 02:30 doesn't exist and is read as 03:30.
        assert_eq!(zone.to_utc(at(2026, 3, 29, 2, 30)), at(2026, 3, 29, 1, 30));
        assert_eq!(zone.to_utc(at(2026, 3, 29, 1, 59)), at(2026, 3, 29, 0, 59));
        assert_eq!(zone.to_utc(at(2026, 3, 29, 3, 0)), at(2026, 3, 29, 1, 0));
        // 25 October 2026, 03:00 → 02:00: 02:30 comes twice; the first is meant.
        assert_eq!(
            zone.to_utc(at(2026, 10, 25, 2, 30)),
            at(2026, 10, 25, 0, 30)
        );
        assert_eq!(zone.to_utc(at(2026, 10, 25, 3, 0)), at(2026, 10, 25, 2, 0));
        assert_eq!(zone.offset_at(at(2026, 10, 25, 0, 59)), 7200);
        assert_eq!(zone.offset_at(at(2026, 10, 25, 1, 0)), 3600);
        assert_eq!(zone.offset_at(at(2027, 1, 5, 0, 0)), 3600);
    }
    // The rules agree with the IANA database every hour of two years.
    let mut t = at(2026, 1, 1, 0, 30);
    while t < at(2028, 1, 1, 0, 0) {
        assert_eq!(rules.to_utc(t), iana.to_utc(t), "{t}");
        assert_eq!(rules.offset_at(t), iana.offset_at(t), "{t}");
        t += TimeDelta::hours(1);
    }
    // The event in the gap shows at 03:30 here, 03:30–04:30... its end is 03:30 written.
    let v = view(&cal, &cal.events[0], &Europe::Warsaw);
    assert_eq!(v.start, Some(at(2026, 3, 29, 3, 30)));
    assert_eq!(v.end, Some(at(2026, 3, 29, 3, 30)));
    assert_eq!(v.zone, None);
}

#[test]
fn old_outlook_rules_and_dates() {
    // Second Sunday of March written as a Sunday on the 8th–14th; an RDATE list.
    let text = "BEGIN:VCALENDAR
BEGIN:VTIMEZONE
TZID:Old US Eastern
BEGIN:DAYLIGHT
DTSTART:20070311T020000
TZOFFSETFROM:-0500
TZOFFSETTO:-0400
RRULE:FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=8,9,10,11,12,13,14;BYDAY=SU
END:DAYLIGHT
BEGIN:STANDARD
DTSTART:20071104T020000
TZOFFSETFROM:-0400
TZOFFSETTO:-0500
RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU
END:STANDARD
END:VTIMEZONE
BEGIN:VTIMEZONE
TZID:Fixed dates
BEGIN:STANDARD
DTSTART:20000101T000000
TZOFFSETFROM:+0300
TZOFFSETTO:+0300
END:STANDARD
BEGIN:DAYLIGHT
DTSTART:20260601T000000
RDATE:20270601T000000,20280601T000000
TZOFFSETFROM:+0300
TZOFFSETTO:+0400
END:DAYLIGHT
BEGIN:STANDARD
DTSTART:20260901T000000
RDATE:20270901T000000
TZOFFSETFROM:+0400
TZOFFSETTO:+0300
END:STANDARD
END:VTIMEZONE
BEGIN:VEVENT
UID:x
END:VEVENT
END:VCALENDAR
";
    let cal = parse(text).unwrap();
    let us = zones::find("Old US Eastern", &cal.zones).unwrap();
    assert_eq!(us.offset_at(at(2026, 3, 8, 6, 59)), -5 * 3600);
    assert_eq!(us.offset_at(at(2026, 3, 8, 7, 0)), -4 * 3600);
    assert_eq!(us.offset_at(at(2026, 11, 1, 6, 0)), -5 * 3600);
    let fixed = zones::find("Fixed dates", &cal.zones).unwrap();
    assert_eq!(fixed.offset_at(at(2026, 7, 1, 0, 0)), 4 * 3600);
    assert_eq!(fixed.offset_at(at(2027, 7, 1, 0, 0)), 4 * 3600);
    assert_eq!(fixed.offset_at(at(2027, 10, 1, 0, 0)), 3 * 3600);
    assert_eq!(fixed.offset_at(at(2025, 7, 1, 0, 0)), 3 * 3600);
    assert!(zones::find("Missing", &cal.zones).is_none());
}

#[test]
fn utc_floating_and_unknown_zones() {
    let text = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:a
DTSTART:20261020T080000Z
DURATION:PT45M
END:VEVENT
BEGIN:VEVENT
UID:b
DTSTART:20261020T080000
DTEND:20261020T090000
END:VEVENT
BEGIN:VEVENT
UID:c
DTSTART;TZID=Somewhere/Unknown:20261020T080000
DTEND;TZID=Somewhere/Unknown:20261020T083000
END:VEVENT
END:VCALENDAR
";
    let cal = parse(text).unwrap();
    let a = view(&cal, &cal.events[0], &Europe::Warsaw);
    assert_eq!(
        (a.start, a.end),
        (
            Some(at(2026, 10, 20, 10, 0)),
            Some(at(2026, 10, 20, 10, 45))
        )
    );
    assert_eq!(a.zone, None, "UTC is nobody's zone to name");
    let b = view(&cal, &cal.events[1], &America::New_York);
    assert_eq!(
        (b.start, b.end),
        (Some(at(2026, 10, 20, 8, 0)), Some(at(2026, 10, 20, 9, 0)))
    );
    let c = view(&cal, &cal.events[2], &America::New_York);
    assert_eq!(
        (c.start, c.end),
        (Some(at(2026, 10, 20, 8, 0)), Some(at(2026, 10, 20, 8, 30)))
    );
}

// ---- Rules in words ----

#[test]
fn repeating_rules_in_words() {
    let wed = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let say = |lang, rule: &str| with(lang, || describe(rule, Some(wed), |m| m.date()));
    let cases = [
        (
            "FREQ=WEEKLY;BYDAY=MO",
            "Every week on Monday",
            "Co tydzień w poniedziałek",
        ),
        (
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=TH,MO",
            "Every 2 weeks on Monday and Thursday",
            "Co 2 tygodnie w poniedziałek i czwartek",
        ),
        (
            "FREQ=WEEKLY;INTERVAL=5;BYDAY=TU",
            "Every 5 weeks on Tuesday",
            "Co 5 tygodni we wtorek",
        ),
        (
            "FREQ=WEEKLY;BYDAY=MO,WE,FR;WKST=SU",
            "Every week on Monday, Wednesday and Friday",
            "Co tydzień w poniedziałek, środę i piątek",
        ),
        (
            "FREQ=WEEKLY",
            "Every week on Wednesday",
            "Co tydzień w środę",
        ),
        (
            "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
            "Every weekday",
            "W dni robocze",
        ),
        (
            "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR",
            "Every weekday",
            "W dni robocze",
        ),
        ("FREQ=DAILY", "Every day", "Codziennie"),
        ("FREQ=DAILY;INTERVAL=3", "Every 3 days", "Co 3 dni"),
        (
            "FREQ=MONTHLY;BYDAY=2TU",
            "Every month on the second Tuesday",
            "Co miesiąc w drugi wtorek",
        ),
        (
            "FREQ=MONTHLY;BYDAY=WE;BYSETPOS=-1",
            "Every month on the last Wednesday",
            "Co miesiąc w ostatnią środę",
        ),
        (
            "FREQ=MONTHLY;BYMONTHDAY=15",
            "Every month on day 15",
            "Co miesiąc, 15. dnia",
        ),
        (
            "FREQ=MONTHLY;INTERVAL=3",
            "Every 3 months on day 21",
            "Co 3 miesiące, 21. dnia",
        ),
        (
            "FREQ=MONTHLY;INTERVAL=6",
            "Every 6 months on day 21",
            "Co 6 miesięcy, 21. dnia",
        ),
        (
            "FREQ=MONTHLY;BYMONTHDAY=-1",
            "Every month on the last day",
            "Co miesiąc, ostatniego dnia",
        ),
        ("FREQ=YEARLY", "Every year", "Co roku"),
        (
            "FREQ=YEARLY;INTERVAL=2;BYMONTH=10;BYMONTHDAY=21",
            "Every 2 years",
            "Co 2 lata",
        ),
        (
            "FREQ=WEEKLY;BYDAY=MO;COUNT=10",
            "Every week on Monday, 10 times",
            "Co tydzień w poniedziałek, 10 razy",
        ),
        (
            "FREQ=WEEKLY;BYDAY=FR;UNTIL=20261231T225959Z",
            "Every week on Friday, until 31 Dec 2026",
            "Co tydzień w piątek, do 31.12.2026",
        ),
        ("FREQ=DAILY;BYHOUR=9,17", "Repeats", "Powtarza się"),
        ("FREQ=MONTHLY;BYDAY=MO,TU", "Repeats", "Powtarza się"),
        ("FREQ=MONTHLY;BYDAY=7MO", "Repeats", "Powtarza się"),
        ("FREQ=HOURLY", "Repeats", "Powtarza się"),
        ("FREQ=SOMETIMES", "Repeats", "Powtarza się"),
        ("FREQ=WEEKLY;BYDAY=MOż", "Repeats", "Powtarza się"),
        ("FREQ=WEEKLY;INTERVAL=0", "Repeats", "Powtarza się"),
    ];
    for (rule, en, pl) in cases {
        assert_eq!(say(Lang::En, rule), en, "{rule}");
        assert_eq!(say(Lang::Pl, rule), pl, "{rule}");
    }
}

// ---- Hostile calendars ----

#[test]
fn hostile_calendars_stay_within_limits() {
    // Many events, many attendees, deep nesting, a huge line, no END lines.
    let mut text = String::from("BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\n");
    for _ in 0..MAX_DEPTH * 3 {
        text.push_str("BEGIN:X-DEEP\r\n");
    }
    for _ in 0..MAX_DEPTH * 3 {
        text.push_str("END:X-DEEP\r\n");
    }
    text.push_str("BEGIN:VEVENT\r\nUID:first\r\nSUMMARY:");
    text.push_str(&"ż".repeat(MAX_LINE));
    text.push_str("\r\n");
    for i in 0..1000 {
        text.push_str(&format!("ATTENDEE;CN=\"P{i}\":mailto:p{i}@x.pl\r\n"));
    }
    text.push_str("END:VEVENT\r\n");
    for i in 0..5000 {
        text.push_str(&format!("BEGIN:VEVENT\r\nUID:{i}\r\nEND:VEVENT\r\n"));
    }
    let cal = parse(&text).expect("still read");
    assert!(cal.events.len() < MAX_COMPONENTS, "{}", cal.events.len());
    let first = &cal.events[0];
    assert_eq!(first.uid, "first");
    assert!(first.summary.len() <= MAX_LINE * 2);
    assert_eq!(first.attendees.len(), MAX_ATTENDEES);
    assert_eq!(first.attendee_count, 1000);
    assert_eq!(
        view(&cal, first, &Europe::Warsaw).more_attendees,
        1000 - MAX_ATTENDEES
    );

    // Cut off in the middle: what was read stands.
    let cut = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:cut\r\nSUMMARY:Pół";
    assert_eq!(parse(cut).unwrap().events[0].summary, "Pół");
    for nothing in [
        "",
        "garbage",
        "BEGIN:VCALENDAR\r\nEND:VCALENDAR",
        "BEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT",
    ] {
        assert!(parse(nothing).is_none(), "{nothing:?}");
    }
    // Too long to read whole: read up to the limit, without trouble.
    let mut big = b"BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:big\r\nDESCRIPTION:".to_vec();
    big.resize(MAX_BYTES * 2, b'a');
    let cal = parse(&decode(&big, None)).unwrap();
    assert!(cal.events[0].description.chars().count() <= MAX_TEXT + 1);
    // Bad dates and times are left out, not guessed.
    let bad = parse("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:b\r\nDTSTART:2026ż0101\r\nDTEND:20261399T250000\r\nSEQUENCE:x\r\nEND:VEVENT\r\nEND:VCALENDAR").unwrap();
    assert_eq!(
        (
            &bad.events[0].start,
            &bad.events[0].end,
            bad.events[0].sequence
        ),
        (&None, &None, 0)
    );
}

// ---- Replies ----

#[test]
fn reply_is_folded_and_keeps_the_event() {
    let raw = outlook_mail(OUTLOOK_ICS, "o1");
    let (_, cal) = calendar_of(&raw).unwrap();
    let e = cal.main().unwrap();
    let me = Person {
        name: "Biuro Studio".into(),
        email: "biuro@twojastudio.pl".into(),
    };
    let comment =
        "Będę, ale spóźnię się 10 minut; proszę zacząć beze mnie, a potem omówimy wycenę.";
    let now = Utc.with_ymd_and_hms(2026, 10, 17, 12, 2, 0).unwrap();
    let ics = reply_ics(&cal, e, &me, Answer::Accepted, comment, now);
    for line in ics.split("\r\n") {
        assert!(line.len() <= 75, "{line:?}");
    }
    let back = parse(&ics).unwrap();
    assert_eq!(back.method, Method::Reply);
    let r = &back.events[0];
    assert_eq!(r.uid, e.uid);
    assert_eq!(r.sequence, 0);
    assert_eq!(r.comment, comment);
    assert_eq!(r.organizer, e.organizer);
    assert_eq!(r.attendees.len(), 1, "only the user's own line");
    assert_eq!(r.attendees[0].person, me);
    assert_eq!(r.attendees[0].answer, Answer::Accepted);
    assert_eq!(r.start, Some(Moment::Utc(at(2026, 10, 20, 8, 0))));
    assert_eq!(r.end, Some(Moment::Utc(at(2026, 10, 20, 9, 0))));
    assert_eq!(r.stamp, Some(Moment::Utc(at(2026, 10, 17, 12, 2))));
    assert!(ics.contains("ORGANIZER;CN=\"Kowalska, Anna\":mailto:anna@firma.pl"));
    with(Lang::Pl, || {
        assert_eq!(
            reply_subject(Answer::Tentative, "Plan"),
            "Wstępnie zaakceptowano: Plan"
        );
        assert_eq!(
            reply_text(Answer::Declined, "Biuro", "Plan", ""),
            "Biuro odrzuca zaproszenie: Plan\n"
        );
    });
    assert_eq!(
        with(Lang::En, || reply_subject(Answer::Declined, "")),
        "Declined: Invitation"
    );

    // One date of a repeating event: its RECURRENCE-ID goes back, in UTC.
    let one = OUTLOOK_ICS.replace(
        "SEQUENCE:0\n",
        "SEQUENCE:3\nRECURRENCE-ID;TZID=Central European Standard Time:20261027T100000\n",
    );
    let cal = parse(&one).unwrap();
    let ics = reply_ics(&cal, &cal.events[0], &me, Answer::Declined, "", now);
    assert!(
        ics.contains("\r\nRECURRENCE-ID:20261027T090000Z\r\n"),
        "{ics}"
    );
    assert!(ics.contains("\r\nSEQUENCE:3\r\n"));
    assert!(!ics.contains("COMMENT"));
}

// ---- In the mail base ----

fn store() -> (MailStore, crate::model::Account) {
    with_account(MailStore::in_memory().unwrap())
}

fn with_account(mut store: MailStore) -> (MailStore, crate::model::Account) {
    let account = store
        .upsert_account(&AccountSetup {
            name: "Biuro Studio".into(),
            email: "biuro@twojastudio.pl".into(),
            host: "mail.twojastudio.pl".into(),
            port: 993,
            smtp_security: SmtpSecurity::Tls,
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
                folder("spam", FolderKind::Spam),
                folder("trash", FolderKind::Trash),
            ],
        )
        .unwrap();
    let account = store.account(&account.id).unwrap().clone();
    (store, account)
}

fn deliver(
    store: &mut MailStore,
    account: &crate::model::Account,
    folder: &str,
    uid: u32,
    raw: Vec<u8>,
) -> MessageId {
    let key = account.folder_key(folder);
    let mut message = crate::mime::parse_message(&raw, &key, None);
    message.folder = key.clone();
    store
        .add_synced(&key, &[SyncedMessage { uid, message, raw }])
        .unwrap()[0]
}

fn now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 17, 14, 2, 0).unwrap()
}

#[test]
fn answering_an_invitation_then_its_update_and_cancellation() {
    let (mut store, account) = store();
    let first = deliver(
        &mut store,
        &account,
        "inbox",
        1,
        outlook_mail(OUTLOOK_ICS, "o1"),
    );
    let plain = deliver(
        &mut store,
        &account,
        "inbox",
        2,
        b"Subject: x\r\n\r\nNo invite.ics here\r\n".to_vec(),
    );
    assert_eq!(store.invite(plain).unwrap(), None);

    let invite = store
        .invite_in(first, &Europe::Warsaw)
        .unwrap()
        .expect("an invitation");
    assert_eq!(invite.method, Method::Request);
    assert_eq!(invite.state, State::Current);
    assert_eq!(invite.blocked, None);
    assert_eq!(invite.mine, Some(Answer::NeedsAction));
    assert!(invite.event.attendees[0].me && !invite.event.attendees[1].me);
    assert_eq!(invite.answered, None);
    assert!(store.inspect_message(first).unwrap().invite.is_some());

    let replied = with(Lang::En, || {
        store
            .answer_invite(first, Answer::Accepted, "Będę.", now())
            .unwrap()
    });
    let outbox = store.list(&account.folder_key("outbox")).unwrap();
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].id, replied.queued);
    let reply = store.get(replied.queued).unwrap();
    assert_eq!(reply.subject, "Accepted: Przegląd projektu");
    assert_eq!(reply.to, "\"Kowalska, Anna\" <anna@firma.pl>");
    assert_eq!(reply.from_email, "biuro@twojastudio.pl");
    assert!(reply
        .body
        .contains("Biuro Studio accepted the invitation: Przegląd projektu"));
    assert!(reply.body.contains("Będę."));
    let raw = store.raw_of(replied.queued).unwrap().unwrap();
    let headers = String::from_utf8_lossy(&raw);
    assert!(headers.contains("In-Reply-To: <o1@firma.pl>"), "{headers}");
    assert!(
        headers.contains("text/calendar; charset=utf-8; method=REPLY"),
        "{headers}"
    );
    let (_, cal) = calendar_of(&raw).expect("the reply's calendar");
    assert_eq!(cal.method, Method::Reply);
    assert_eq!(cal.events[0].attendees[0].answer, Answer::Accepted);
    assert_eq!(cal.events[0].comment, "Będę.");

    let invite = store.invite_in(first, &Europe::Warsaw).unwrap().unwrap();
    let answered = invite.answered.expect("answered");
    assert_eq!(
        (answered.answer, answered.earlier),
        (Answer::Accepted, false)
    );
    assert_eq!(
        store.get(first).unwrap().note.as_deref(),
        Some("You accepted on 17 Oct 2026, 14:02.")
    );

    // The organizer moves it: SEQUENCE 1. The first is outdated; the new one was answered
    // in an earlier version.
    let moved = OUTLOOK_ICS
        .replace("SEQUENCE:0", "SEQUENCE:1")
        .replace("20261020T100000", "20261020T120000");
    let second = deliver(&mut store, &account, "inbox", 3, outlook_mail(&moved, "o2"));
    let old = store.invite_in(first, &Europe::Warsaw).unwrap().unwrap();
    assert_eq!(
        (old.state, old.blocked),
        (State::Outdated, Some(Blocked::Outdated))
    );
    assert!(store
        .answer_invite(first, Answer::Declined, "", now())
        .is_err());
    let new = store.invite_in(second, &Europe::Warsaw).unwrap().unwrap();
    assert_eq!(new.state, State::Current);
    assert_eq!(new.event.start, Some(at(2026, 10, 20, 12, 0)));
    assert!(new.answered.unwrap().earlier);

    // Then it is called off.
    let off = OUTLOOK_ICS
        .replace("METHOD:REQUEST", "METHOD:CANCEL")
        .replace("SEQUENCE:0", "SEQUENCE:2")
        .replace("STATUS:CONFIRMED", "STATUS:CANCELLED");
    let cancel = deliver(&mut store, &account, "inbox", 4, outlook_mail(&off, "o3"));
    for id in [first, second, cancel] {
        let i = store.invite_in(id, &Europe::Warsaw).unwrap().unwrap();
        assert_eq!(i.state, State::Cancelled, "{id}");
    }
    assert_eq!(
        store.invite(cancel).unwrap().unwrap().blocked,
        Some(Blocked::NotInvitation)
    );
}

#[test]
fn no_answers_from_spam_or_to_ones_own_meeting() {
    let (mut store, account) = store();
    let spam = deliver(&mut store, &account, "spam", 1, google_mail());
    let invite = store.invite(spam).unwrap().unwrap();
    assert!(invite.spam);
    assert_eq!(invite.blocked, Some(Blocked::Spam));
    let refused = with(Lang::En, || {
        store.answer_invite(spam, Answer::Accepted, "", now())
    });
    assert!(refused.unwrap_err().contains("Spam"));
    assert!(store
        .list(&account.folder_key("outbox"))
        .unwrap()
        .is_empty());

    let own = OUTLOOK_ICS.replace("mailto:anna@firma.pl", "mailto:biuro@twojastudio.pl");
    let mine = deliver(&mut store, &account, "inbox", 2, outlook_mail(&own, "own"));
    assert_eq!(
        store.invite(mine).unwrap().unwrap().blocked,
        Some(Blocked::Organizer)
    );

    // Someone's answer to the user's own invitation.
    let answer = OUTLOOK_ICS
        .replace("METHOD:REQUEST", "METHOD:REPLY")
        .replace("PARTSTAT=NEEDS-ACTION", "PARTSTAT=DECLINED");
    let reply = deliver(
        &mut store,
        &account,
        "inbox",
        3,
        outlook_mail(&answer, "r1"),
    );
    let invite = store.invite(reply).unwrap().unwrap();
    assert_eq!(invite.method, Method::Reply);
    let from = invite.from.unwrap();
    assert_eq!(
        (from.person.email.as_str(), from.answer),
        ("biuro@twojastudio.pl", Answer::Declined)
    );
}

/// Mail stored before schema 18, as an upgrade leaves it: no invitations noted, a job queued.
fn as_before_the_upgrade(store: &MailStore) {
    store.db.execute("DELETE FROM invites", []).unwrap();
    crate::upkeep::queue(&store.db, crate::upkeep::INVITES).unwrap();
}

fn noted(store: &MailStore) -> Vec<MessageId> {
    let mut stmt = store
        .db
        .prepare("SELECT message FROM invites ORDER BY message")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, i64>(0))
        .unwrap()
        .map(|id| id.unwrap() as MessageId)
        .collect()
}

#[test]
fn the_upkeep_finds_invitations_already_stored() {
    let (mut store, account) = store();
    let id = deliver(&mut store, &account, "inbox", 1, apple_mail());
    as_before_the_upgrade(&store);
    assert_eq!(
        store.upkeep_left().unwrap(),
        Some(crate::upkeep::Progress { done: 0, total: 1 })
    );
    assert_eq!(store.upkeep_step(Duration::MAX).unwrap(), None);
    assert_eq!(noted(&store), [id]);
    let invite = store.invite(id).unwrap().unwrap();
    assert_eq!(invite.event.summary, "Święta u rodziców");
    assert_eq!(invite.blocked, None);
    assert_eq!(store.upkeep_left().unwrap(), None, "the job is gone");
}

#[test]
fn the_upkeep_goes_on_where_it_stopped() {
    let path = std::env::temp_dir().join(format!(
        "telex-upkeep-{}-{}.db",
        std::process::id(),
        Local::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let (mut store, account) = with_account(MailStore::open(&path).unwrap());
    let oldest = deliver(&mut store, &account, "inbox", 1, apple_mail());
    let plain = deliver(
        &mut store,
        &account,
        "inbox",
        2,
        b"From: a@x.pl\r\nSubject: Hi\r\nMessage-ID: <p@x.pl>\r\n\r\nNo calendar here.\r\n"
            .to_vec(),
    );
    let newest = deliver(&mut store, &account, "inbox", 3, google_mail());
    as_before_the_upgrade(&store);

    // A step that is over at once still does one message: the newest first.
    let left = store.upkeep_step(Duration::ZERO).unwrap();
    assert_eq!(left, Some(crate::upkeep::Progress { done: 1, total: 3 }));
    assert_eq!(noted(&store), [newest]);
    // Telex closes here; the next start goes on with the message after it.
    drop(store);
    let mut store = MailStore::open(&path).unwrap();
    assert_eq!(
        store.upkeep_left().unwrap(),
        Some(crate::upkeep::Progress { done: 1, total: 3 })
    );
    let left = store.upkeep_step(Duration::ZERO).unwrap();
    assert_eq!(left, Some(crate::upkeep::Progress { done: 2, total: 3 }));
    assert!(!noted(&store).contains(&plain));
    // Mail that comes meanwhile is noted as it is stored, and the job doesn't count it.
    let arrived = deliver(
        &mut store,
        &account,
        "inbox",
        4,
        outlook_mail(OUTLOOK_ICS, "new"),
    );
    assert_eq!(store.upkeep_step(Duration::ZERO).unwrap(), None);
    assert_eq!(noted(&store), [oldest, newest, arrived]);
    drop(store);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[test]
fn an_invitation_opened_before_the_upkeep_got_there_shows() {
    let (mut store, account) = store();
    let old = deliver(&mut store, &account, "inbox", 1, apple_mail());
    let plain = deliver(
        &mut store,
        &account,
        "inbox",
        2,
        b"From: a@x.pl\r\nSubject: Hi\r\n\r\nNo calendar here.\r\n".to_vec(),
    );
    as_before_the_upgrade(&store);
    assert!(noted(&store).is_empty());
    let invite = store.invite(old).unwrap().expect("noted when opened");
    assert_eq!(invite.event.summary, "Święta u rodziców");
    assert_eq!(noted(&store), [old]);
    assert!(store.invite(plain).unwrap().is_none());
    // The job still goes over it, without noting it twice.
    while store.upkeep_step(Duration::ZERO).unwrap().is_some() {}
    assert_eq!(noted(&store), [old]);
}
