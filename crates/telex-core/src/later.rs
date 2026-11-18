//! Send later: when a message the user scheduled goes out.
//!
//! A scheduled message waits in the Outbox with `outbox.send_after`, like one waiting out
//! its moment to undo; [`MailStore::next_scheduled`](crate::MailStore::next_scheduled) says
//! when the next one is due, and the app sends it then (or at the next start, when Telex was
//! closed: [`late`] tells the user it went late). Times are local, as on the user's clock.

use jiff::civil::{DateTime, Weekday};
use jiff::{SignedDuration, ToSpan};
use serde::{Deserialize, Serialize};

use crate::tr;

/// "Morning" is 08:00.
pub const MORNING: i8 = 8;

/// A message that goes this long after its time went late: Telex was closed, or asleep.
pub const LATE_AFTER: SignedDuration = SignedDuration::from_mins(2);

/// The choices of the Send later menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "at", rename_all = "snake_case")]
pub enum Later {
    /// In an hour, to the minute.
    Hour,
    /// Tomorrow at 08:00.
    Tomorrow,
    /// The coming Monday at 08:00 (a week from today on a Monday).
    Monday,
    /// A time the user picked.
    At(DateTime),
}

/// One line of the menu: what it is and when that is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Choice {
    #[serde(flatten)]
    pub later: Later,
    pub when: DateTime,
}

/// When `later` is, seen from `now`. A picked time must be at least a minute ahead.
pub fn when(later: Later, now: DateTime) -> Result<DateTime, String> {
    let morning = |days: i64| (now.date() + days.days()).at(MORNING, 0, 0, 0);
    Ok(match later {
        Later::Hour => to_minute(now + SignedDuration::from_hours(1)),
        Later::Tomorrow => morning(1),
        Later::Monday => {
            let ahead = 7 - i64::from(now.weekday().to_monday_zero_offset());
            morning(ahead)
        }
        Later::At(at) => {
            let at = to_minute(at);
            if at < now + SignedDuration::from_mins(1) {
                return Err(tr!(
                    "Pick a time later than now.",
                    "Wybierz godzinę późniejszą niż teraz."
                ));
            }
            at
        }
    })
}

/// The menu's choices as they are now. Monday is left out when it is tomorrow.
pub fn choices(now: DateTime) -> Vec<Choice> {
    let mut out = Vec::new();
    for later in [Later::Hour, Later::Tomorrow, Later::Monday] {
        let Ok(at) = when(later, now) else { continue };
        if later == Later::Monday && now.weekday() == Weekday::Sunday {
            continue;
        }
        out.push(Choice { later, when: at });
    }
    out
}

/// True when a message due at `due` and sent at `sent` went late.
pub fn late(due: DateTime, sent: DateTime) -> bool {
    sent.duration_since(due) > LATE_AFTER
}

/// "19.10 08:00": the time a message was due, for a sentence.
pub fn short(at: DateTime) -> String {
    at.strftime("%d.%m %H:%M").to_string()
}

fn to_minute(at: DateTime) -> DateTime {
    at.with()
        .second(0)
        .subsec_nanosecond(0)
        .build()
        .unwrap_or(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(d: i8, h: i8, m: i8) -> DateTime {
        // October 2026: the 17th is a Saturday, the 18th a Sunday, the 19th a Monday.
        jiff::civil::date(2026, 10, d).at(h, m, 0, 0)
    }

    #[test]
    fn an_hour_tomorrow_and_monday() {
        let now = at(17, 14, 5) + SignedDuration::from_secs(42);
        assert_eq!(when(Later::Hour, now).unwrap(), at(17, 15, 5));
        assert_eq!(when(Later::Tomorrow, now).unwrap(), at(18, 8, 0));
        assert_eq!(when(Later::Monday, now).unwrap(), at(19, 8, 0));
        // Late in the evening, an hour goes over midnight.
        assert_eq!(when(Later::Hour, at(17, 23, 30)).unwrap(), at(18, 0, 30));
    }

    #[test]
    fn monday_on_a_monday_is_next_week() {
        assert_eq!(when(Later::Monday, at(19, 6, 0)).unwrap(), at(26, 8, 0));
        assert_eq!(when(Later::Monday, at(25, 22, 0)).unwrap(), at(26, 8, 0));
    }

    #[test]
    fn a_picked_time_must_be_ahead() {
        let now = at(17, 14, 0);
        assert_eq!(
            when(
                Later::At(at(20, 9, 30) + SignedDuration::from_secs(15)),
                now
            )
            .unwrap(),
            at(20, 9, 30)
        );
        assert!(when(Later::At(at(17, 13, 0)), now).is_err());
        assert!(when(Later::At(now), now).is_err());
        let error = crate::i18n::with(crate::i18n::Lang::Pl, || {
            when(Later::At(at(1, 0, 0)), now).unwrap_err()
        });
        assert_eq!(error, "Wybierz godzinę późniejszą niż teraz.");
    }

    #[test]
    fn the_menu_leaves_out_monday_when_it_is_tomorrow() {
        let kinds = |now| choices(now).iter().map(|c| c.later).collect::<Vec<_>>();
        assert_eq!(
            kinds(at(17, 10, 0)),
            [Later::Hour, Later::Tomorrow, Later::Monday]
        );
        assert_eq!(kinds(at(18, 10, 0)), [Later::Hour, Later::Tomorrow]);
        let json = serde_json::to_value(choices(at(17, 10, 0))[1]).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "tomorrow", "when": "2026-10-18T08:00:00"})
        );
        let picked: Later =
            serde_json::from_value(serde_json::json!({"kind": "at", "at": "2026-10-20T09:30:00"}))
                .unwrap();
        assert_eq!(picked, Later::At(at(20, 9, 30)));
    }

    #[test]
    fn late_is_more_than_two_minutes() {
        let due = at(19, 8, 0);
        assert!(!late(due, due + SignedDuration::from_secs(90)));
        assert!(late(due, due + SignedDuration::from_mins(3)));
        assert!(late(due, at(20, 7, 0)));
        assert_eq!(short(due), "19.10 08:00");
    }
}
