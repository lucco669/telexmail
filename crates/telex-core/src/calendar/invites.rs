//! Invitations in the mail base: which messages hold one (the `invites` table, filled as mail
//! is stored), what the card says about one (outdated, cancelled, answered), and answering.
//!
//! Every message stored goes past [`index`]: mail that names a calendar anywhere (a cheap
//! byte search) is parsed, and its event's UID, SEQUENCE, RECURRENCE-ID and METHOD are kept.
//! A newer SEQUENCE of the same UID makes older invitations outdated, a CANCEL cancels them,
//! and the user's own REPLY (in the Outbox or Sent, also one written by another program)
//! is their answer.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use mail_parser::{Encoding, MessageParser, MessagePart, MimeHeaders};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::{
    decode, parse, reply_ics, reply_subject, reply_text, view, Answer, Attendee, Calendar,
    EventView, Method, Moment, Person,
};
use crate::drafts;
use crate::model::{account_of, FolderKind, MessageId, OutgoingDraft};
use crate::smtp::{self, Originals};
use crate::store::{MailStore, StoreError};
use crate::tr;
use crate::upkeep;

/// What the card says about an invitation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invite {
    pub method: Method,
    pub event: EventView,
    /// The attachment holding the calendar (as `get_message` lists them): Add to calendar
    /// opens it, and it can be saved.
    pub file: usize,
    pub state: State,
    /// The user's answer as the invitation has it (their ATTENDEE line's PARTSTAT).
    pub mine: Option<Answer>,
    /// The user's last reply to this event from Telex or another program (in the Outbox or
    /// Sent).
    pub answered: Option<Answered>,
    /// Why it can't be answered here; `None` when Accept, Maybe and Decline work.
    pub blocked: Option<Blocked>,
    /// A reply to the user's own invitation: who answered, and how.
    pub from: Option<Attendee>,
    /// It is in Spam: no answering, links not clickable.
    pub spam: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Current,
    /// A newer version of this invitation came.
    Outdated,
    /// The organizer called it off (this message, or a later one).
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Answered {
    pub answer: Answer,
    /// When (the reply's date, local time).
    pub at: NaiveDateTime,
    /// It answered an earlier version of the invitation.
    pub earlier: bool,
}

/// Why an invitation can't be answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocked {
    /// Nothing to answer: a cancellation, a reply, an event to look at.
    NotInvitation,
    Spam,
    Cancelled,
    Outdated,
    /// The user organizes it.
    Organizer,
    /// The invitation names no organizer to answer.
    NoOrganizer,
    /// No account here can send.
    NoAccount,
}

impl Blocked {
    fn sentence(self) -> String {
        match self {
            Self::NotInvitation => tr!(
                "This isn't an invitation to answer.",
                "To nie jest zaproszenie, na które można odpowiedzieć."
            ),
            Self::Spam => tr!(
                "Invitations in Spam aren't answered. Move it out of Spam first.",
                "Na zaproszenia w Spamie nie można odpowiadać. Najpierw przenieś je ze Spamu."
            ),
            Self::Cancelled => tr!("This meeting was cancelled.", "To spotkanie odwołano."),
            Self::Outdated => tr!(
                "A newer version of this invitation came. Answer that one.",
                "Przyszła nowsza wersja tego zaproszenia. Odpowiedz na nią."
            ),
            Self::Organizer => tr!(
                "You organize this meeting.",
                "To ty organizujesz to spotkanie."
            ),
            Self::NoOrganizer => tr!(
                "The invitation doesn't say who organizes it.",
                "Zaproszenie nie podaje organizatora."
            ),
            Self::NoAccount => tr!(
                "Add an account to answer from.",
                "Dodaj konto, z którego można odpowiedzieć."
            ),
        }
    }
}

/// What answering did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Replied {
    /// The reply, in the Outbox.
    pub queued: MessageId,
    pub account: String,
}

/// Does the raw message name a calendar anywhere (text/calendar, application/ics, .ics, in any
/// case)? A byte search, so most mail is never parsed: one pass for the "/" and "." these have
/// (SIMD), each looked at around it; an upgrade runs it over all mail (upkeep.rs).
pub(super) fn mentions_calendar(raw: &[u8]) -> bool {
    let starts = |at: usize, word: &[u8]| {
        raw.get(at..at + word.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(word))
    };
    let ends = |at: usize, word: &[u8]| {
        at.checked_sub(word.len())
            .is_some_and(|from| raw[from..at].eq_ignore_ascii_case(word))
    };
    memchr::memchr2_iter(b'/', b'.', raw).any(|i| {
        if raw[i] == b'.' {
            starts(i + 1, b"ics")
        } else {
            (ends(i, b"text") && starts(i + 1, b"calendar"))
                || (ends(i, b"application") && starts(i + 1, b"ics"))
        }
    })
}

/// A part's bytes after its transfer encoding, before any charset: folded lines can then be
/// joined before the text is decoded.
fn part_bytes(raw: &[u8], part: &MessagePart<'_>) -> Option<Vec<u8>> {
    let body = raw.get(part.offset_body as usize..part.offset_end as usize)?;
    match part.encoding {
        Encoding::None => Some(body.to_vec()),
        Encoding::QuotedPrintable => {
            mail_parser::decoders::quoted_printable::quoted_printable_decode(body)
        }
        Encoding::Base64 => mail_parser::decoders::base64::base64_decode(body),
    }
}

/// The calendar of a message and which of its attachments holds it: a text/calendar part
/// first (Outlook's and Google's), else an .ics file.
pub fn calendar_of(raw: &[u8]) -> Option<(usize, Calendar)> {
    if !mentions_calendar(raw) {
        return None;
    }
    let p = MessageParser::default().parse(raw)?;
    let mut file = None;
    for (index, part) in p.attachments().enumerate() {
        let kind = part
            .content_type()
            .map(|ct| {
                format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or_default()).to_ascii_lowercase()
            })
            .unwrap_or_default();
        let named_ics = part
            .attachment_name()
            .is_some_and(|n| n.to_ascii_lowercase().ends_with(".ics"));
        let inline = kind == "text/calendar";
        if !inline && !named_ics && kind != "application/ics" && kind != "text/x-vcalendar" {
            continue;
        }
        let charset = part.content_type().and_then(|ct| ct.attribute("charset"));
        let bytes = part_bytes(raw, part).unwrap_or_else(|| part.contents().to_vec());
        let Some(cal) = parse(&decode(&bytes, charset)) else {
            continue;
        };
        if inline {
            return Some((index, cal));
        }
        file.get_or_insert((index, cal));
    }
    file
}

/// RECURRENCE-ID as a key: the UTC time when it can be worked out, else as written; ""
/// for the whole series.
fn recurrence_key(cal: &Calendar, e: &super::Event) -> String {
    let Some(prop) = &e.recurrence else {
        return String::new();
    };
    match Moment::read(prop) {
        Some(Moment::Date(d)) => d.format("%Y%m%d").to_string(),
        Some(Moment::Floating(at)) => at.format("%Y%m%dT%H%M%S").to_string(),
        Some(m) => match cal.instant(&m) {
            Some((utc, _)) => utc.format("%Y%m%dT%H%M%SZ").to_string(),
            None => prop.value.trim().to_owned(),
        },
        None => prop.value.trim().to_owned(),
    }
}

fn stamp_of(cal: &Calendar, e: &super::Event) -> Option<NaiveDateTime> {
    let m = e.stamp.as_ref()?;
    match m {
        Moment::Utc(at) | Moment::Floating(at) => Some(*at),
        Moment::Zoned(..) => cal.instant(m).map(|(u, _)| u),
        Moment::Date(d) => d.and_hms_opt(0, 0, 0),
    }
}

/// Note a stored message's invitation, if it holds one. Called for every message stored, and
/// for the mail stored before schema 18 by the upkeep after an upgrade (upkeep.rs).
pub(crate) fn index(db: &Connection, id: MessageId, raw: Option<&[u8]>) -> Result<(), StoreError> {
    let Some((_, cal)) = raw.and_then(calendar_of) else {
        return Ok(());
    };
    let Some(e) = cal.main().filter(|e| !e.uid.is_empty()) else {
        return Ok(());
    };
    let answer = (cal.method == Method::Reply)
        .then(|| e.attendees.first().map(|a| a.answer.code()))
        .flatten();
    db.prepare_cached(
        "INSERT OR REPLACE INTO invites (message, uid, recurrence, sequence, method, stamp, answer)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        id as i64,
        e.uid,
        recurrence_key(&cal, e),
        e.sequence,
        cal.method.code(),
        stamp_of(&cal, e),
        answer,
    ])?;
    Ok(())
}

/// Another message about the same event.
struct Other {
    sequence: i64,
    method: Method,
    stamp: Option<NaiveDateTime>,
    answer: Option<Answer>,
    folder: String,
    date: NaiveDateTime,
    recurrence: String,
}

impl MailStore {
    /// The invitation in a message, or `None` when it holds none; times in this PC's zone.
    pub fn invite(&self, id: MessageId) -> Result<Option<Invite>, StoreError> {
        self.invite_in(id, &Local)
    }

    /// The invitation in a message, with times in `local`.
    pub fn invite_in<L: TimeZone>(
        &self,
        id: MessageId,
        local: &L,
    ) -> Result<Option<Invite>, StoreError> {
        let noted = || {
            self.db
                .query_row(
                    "SELECT uid, recurrence, sequence, stamp FROM invites WHERE message = ?1",
                    [id as i64],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()
        };
        let mut row: Option<(String, String, i64, Option<NaiveDateTime>)> = noted()?;
        // Mail stored before schema 18 that the upkeep hasn't reached yet: noted now.
        if row.is_none() && upkeep::waiting(&self.db, upkeep::INVITES, id)? {
            index(&self.db, id, self.raw_of(id)?.as_deref())?;
            row = noted()?;
        }
        let Some((uid, recurrence, sequence, stamp)) = row else {
            return Ok(None);
        };
        let Some(raw) = self.raw_of(id)? else {
            return Ok(None);
        };
        let Some((file, cal)) = calendar_of(&raw) else {
            return Ok(None);
        };
        let Some(e) = cal.main() else {
            return Ok(None);
        };
        let message = self.get(id)?;
        let spam = self.kind_of(&message.folder) == Some(FolderKind::Spam);
        let account = drafts::sender(self, Some(&message), account_of(&message.folder)).ok();
        let own: Vec<String> = match account {
            Some(a) => std::iter::once(a.email.to_ascii_lowercase())
                .chain(a.identities.iter().map(|i| i.email.to_ascii_lowercase()))
                .collect(),
            None => Vec::new(),
        };
        let mut event = view(&cal, e, local);
        for a in &mut event.attendees {
            a.me = own.contains(&a.person.email);
        }
        let mine = event.attendees.iter().find(|a| a.me).map(|a| a.answer);

        let others: Vec<Other> = {
            let mut stmt = self.db.prepare_cached(
                "SELECT i.sequence, i.method, i.stamp, i.answer, m.folder, m.date, i.recurrence
                 FROM invites i JOIN messages m ON m.id = i.message
                 WHERE i.uid = ?1 AND i.message != ?2",
            )?;
            let rows = stmt
                .query_map(params![uid, id as i64], |r| {
                    Ok(Other {
                        sequence: r.get(0)?,
                        method: Method::from_code(&r.get::<_, String>(1)?),
                        stamp: r.get(2)?,
                        answer: r
                            .get::<_, Option<String>>(3)?
                            .map(|a| Answer::read(Some(&a))),
                        folder: r.get(4)?,
                        date: r.get(5)?,
                        recurrence: r.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let newer = |o: &Other| {
            o.sequence > sequence
                || (o.sequence == sequence
                    && matches!((o.stamp, stamp), (Some(a), Some(b)) if a > b))
        };
        let outgoing = |o: &Other| {
            matches!(
                self.kind_of(&o.folder),
                Some(FolderKind::Outbox | FolderKind::Sent)
            )
        };
        let cancelled = cal.method == Method::Cancel
            || e.cancelled
            || others.iter().any(|o| {
                o.method == Method::Cancel
                    && o.sequence >= sequence
                    && (o.recurrence == recurrence || o.recurrence.is_empty())
            });
        let outdated = others
            .iter()
            .any(|o| o.method == Method::Request && o.recurrence == recurrence && newer(o));
        let state = if cancelled {
            State::Cancelled
        } else if outdated {
            State::Outdated
        } else {
            State::Current
        };
        let account_id = account.map(|a| a.id.as_str()).unwrap_or_default();
        let answered = others
            .iter()
            .filter(|o| {
                o.method == Method::Reply
                    && o.recurrence == recurrence
                    && outgoing(o)
                    && account_of(&o.folder) == account_id
            })
            .max_by_key(|o| o.date)
            .and_then(|o| {
                Some(Answered {
                    answer: o.answer?,
                    at: o.date,
                    earlier: o.sequence < sequence,
                })
            });
        let organizer = e.organizer.as_ref().filter(|o| !o.email.is_empty());
        let blocked = if cal.method != Method::Request {
            Some(Blocked::NotInvitation)
        } else if spam {
            Some(Blocked::Spam)
        } else if state == State::Cancelled {
            Some(Blocked::Cancelled)
        } else if state == State::Outdated {
            Some(Blocked::Outdated)
        } else if account.is_none() {
            Some(Blocked::NoAccount)
        } else if organizer.is_none() {
            Some(Blocked::NoOrganizer)
        } else if organizer.is_some_and(|o| own.contains(&o.email)) {
            Some(Blocked::Organizer)
        } else {
            None
        };
        let from = (cal.method == Method::Reply)
            .then(|| event.attendees.first().cloned())
            .flatten();
        Ok(Some(Invite {
            method: cal.method,
            event,
            file,
            state,
            mine,
            answered,
            blocked,
            from,
            spam,
        }))
    }

    /// Answer an invitation: an iTIP REPLY to its organizer, from the identity it was sent to,
    /// put in the Outbox (the caller sends it), and a note on the invitation ("You accepted on
    /// …"). Refused in Spam, for cancelled or outdated invitations and the user's own.
    pub fn answer_invite(
        &mut self,
        id: MessageId,
        answer: Answer,
        comment: &str,
        now: DateTime<Local>,
    ) -> Result<Replied, String> {
        if !matches!(
            answer,
            Answer::Accepted | Answer::Tentative | Answer::Declined
        ) {
            return Err(Blocked::NotInvitation.sentence());
        }
        let err = |e: StoreError| e.to_string();
        let invite = self
            .invite(id)
            .map_err(err)?
            .ok_or_else(|| Blocked::NotInvitation.sentence())?;
        if let Some(blocked) = invite.blocked {
            return Err(blocked.sentence());
        }
        let raw = self.raw_of(id).map_err(err)?.unwrap_or_default();
        let (_, cal) = calendar_of(&raw).ok_or_else(|| Blocked::NotInvitation.sentence())?;
        let e = cal
            .main()
            .ok_or_else(|| Blocked::NotInvitation.sentence())?;
        let organizer = e
            .organizer
            .clone()
            .ok_or_else(|| Blocked::NoOrganizer.sentence())?;
        let message = self.get(id).map_err(err)?;
        let account = drafts::sender(self, Some(&message), account_of(&message.folder))
            .map_err(|_| Blocked::NoAccount.sentence())?
            .clone();
        // The attendee line that is the user's, and the identity of that address.
        let own = |email: &str| {
            account.email.eq_ignore_ascii_case(email)
                || account
                    .identities
                    .iter()
                    .any(|i| i.email.eq_ignore_ascii_case(email))
        };
        let attendee = e.attendees.iter().find(|a| own(&a.person.email));
        let identity = attendee
            .and_then(|a| {
                account
                    .identities
                    .iter()
                    .find(|i| i.email.eq_ignore_ascii_case(&a.person.email))
                    .cloned()
            })
            .unwrap_or_else(|| drafts::identity_for(&account, &message));
        let me = Person {
            name: attendee
                .map(|a| a.person.name.trim())
                .filter(|n| !n.is_empty())
                .unwrap_or(identity.name.trim())
                .to_owned(),
            email: attendee.map_or_else(
                || identity.email.to_ascii_lowercase(),
                |a| a.person.email.clone(),
            ),
        };
        let ics = reply_ics(&cal, e, &me, answer, comment, now.with_timezone(&Utc));
        let who = if me.name.is_empty() {
            &me.email
        } else {
            &me.name
        };
        let draft = OutgoingDraft {
            account: account.id.clone(),
            to: organizer.mailbox(),
            subject: reply_subject(answer, &e.summary),
            body: reply_text(answer, who, &e.summary, comment),
            reply_to: Some(id),
            identity: (identity.id != 0).then_some(identity.id),
            ..OutgoingDraft::default()
        };
        let originals = Originals {
            replied: Some(raw),
            forwarded: None,
            ..Originals::default()
        };
        let composed = smtp::compose_invite_reply(&account, &draft, &originals, &ics, now)?;
        let queued = self
            .queue_outgoing(
                &account.id,
                &composed.message,
                &composed.raw,
                &composed.env_from,
                &composed.env_to,
                None,
                None,
            )
            .map_err(err)?;
        // An answer is a reply: the invitation is answered once it went.
        self.note_answering(queued, Some(id), None).map_err(err)?;
        self.add_note(id, &answer.note(now.naive_local()))
            .map_err(err)?;
        Ok(Replied {
            queued,
            account: account.id,
        })
    }
}
