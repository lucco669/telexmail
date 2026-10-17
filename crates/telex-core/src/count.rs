//! Counting copies of Telex, only with the user's yes (`settings::UsageCount::On`).
//!
//! Once a day a copy that may be counted sends one small JSON object to the site
//! (`ENDPOINT`): Telex's version, the system, the interface's language, the month it was first
//! counted, and whether this is its first count ever, this week and this month. Nothing names
//! the copy or the person: there is no ID, so the site can add up copies (a day's counts are
//! the copies used that day; the "first this week" ones, the copies used that week) but can't
//! follow one. Nothing about mail, accounts or settings is sent.
//!
//! Days, weeks and months are UTC, as the site counts them; weeks are ISO weeks. What was sent
//! last is kept in `count.json` next to the settings ([`Sent`]).

use std::path::Path;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

/// The one address a count goes to.
pub const ENDPOINT: &str = "https://telexmail.app/api/count";

/// What this copy has sent, kept in `count.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    /// The UTC day of the last count the site took.
    pub last: Option<NaiveDate>,
    /// The month of the first one, "2026-10".
    pub since: Option<String>,
}

impl Sent {
    /// `count.json`, or nothing sent yet when there is none or it can't be read.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Written whole or not at all, like settings.json.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string(self).map_err(std::io::Error::other)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, text + "\n")?;
        std::fs::rename(&temp, path)
    }

    /// The site took `ping` on `today`.
    pub fn record(&mut self, ping: &Ping, today: NaiveDate) {
        self.last = Some(today);
        self.since = Some(ping.since.clone());
    }
}

/// Everything one count sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ping {
    /// "0.9.10".
    pub version: String,
    /// One of [`OSES`].
    pub os: &'static str,
    /// The interface's language, "en" or "pl".
    pub lang: &'static str,
    /// The month this copy was first counted, "2026-10".
    pub since: String,
    /// The first count of this copy.
    pub new: bool,
    /// The first count of this copy this week, and this month.
    pub week: bool,
    pub month: bool,
}

/// The systems a count can name: Windows, Linux running as an AppImage, other Linux (the .deb).
pub const OSES: &[&str] = &["windows", "appimage", "linux"];

/// This system, as a count names it.
pub fn os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if std::env::var_os("APPIMAGE").is_some() {
        "appimage"
    } else {
        "linux"
    }
}

/// The count due `today`, or `None` when today's was sent already (or the clock went back).
pub fn due(
    sent: &Sent,
    today: NaiveDate,
    version: &str,
    os: &'static str,
    lang: &'static str,
) -> Option<Ping> {
    if sent.last.is_some_and(|last| last >= today) {
        return None;
    }
    let week = |d: NaiveDate| (d.iso_week().year(), d.iso_week().week());
    let month = |d: NaiveDate| (d.year(), d.month());
    Some(Ping {
        version: version.to_owned(),
        os,
        lang,
        since: sent
            .since
            .clone()
            .filter(|s| valid_month(s))
            .unwrap_or_else(|| today.format("%Y-%m").to_string()),
        new: sent.last.is_none(),
        week: sent.last.is_none_or(|last| week(last) != week(today)),
        month: sent.last.is_none_or(|last| month(last) != month(today)),
    })
}

fn valid_month(s: &str) -> bool {
    NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d").is_ok() && s.len() == 7
}

/// Send one count. Ok when the site took it; the next try is the next due one.
pub fn send(endpoint: &str, ping: &Ping) -> Result<(), String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .https_only(true)
        .max_redirects(0)
        .user_agent("Telex")
        .build();
    let body = serde_json::to_string(ping).map_err(|e| e.to_string())?;
    ureq::Agent::new_with_config(config)
        .post(endpoint)
        .header("Content-Type", "application/json")
        .send(body)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn after(last: &str, since: &str) -> Sent {
        Sent {
            last: Some(day(last)),
            since: Some(since.into()),
        }
    }

    #[test]
    fn the_first_count_is_new_in_every_sense() {
        let ping = due(
            &Sent::default(),
            day("2026-10-17"),
            "0.9.10",
            "windows",
            "pl",
        )
        .unwrap();
        assert_eq!(
            ping,
            Ping {
                version: "0.9.10".into(),
                os: "windows",
                lang: "pl",
                since: "2026-10".into(),
                new: true,
                week: true,
                month: true,
            }
        );
    }

    #[test]
    fn once_a_day() {
        let sent = after("2026-10-17", "2026-10");
        assert_eq!(due(&sent, day("2026-10-17"), "0.9.10", "linux", "en"), None);
        // A clock set back sends nothing until the day comes again.
        assert_eq!(due(&sent, day("2026-10-16"), "0.9.10", "linux", "en"), None);
        let next = due(&sent, day("2026-10-18"), "0.9.10", "linux", "en").unwrap();
        assert!(!next.new);
    }

    #[test]
    fn weeks_and_months_say_when_they_begin() {
        // 2026-10-17 is a Saturday, 10-18 the Sunday of the same ISO week, 10-19 a Monday.
        let sent = after("2026-10-17", "2026-09");
        let sunday = due(&sent, day("2026-10-18"), "0.9.10", "windows", "en").unwrap();
        assert!(!sunday.week && !sunday.month);
        let monday = due(&sent, day("2026-10-19"), "0.9.10", "windows", "en").unwrap();
        assert!(monday.week && !monday.month);
        // Monday 26 October and Sunday 1 November are both in week 44.
        let late = after("2026-10-26", "2026-09");
        let november = due(&late, day("2026-11-01"), "0.9.10", "windows", "en").unwrap();
        assert!(!november.week && november.month);
        assert_eq!(november.since, "2026-09", "the first month stays");
        // The same week number a year later is another week.
        let year = due(&sent, day("2027-10-16"), "0.9.10", "windows", "en").unwrap();
        assert!(year.week && year.month);
    }

    #[test]
    fn what_was_sent_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("telex-count-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("count.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(Sent::load(&path), Sent::default());
        let today = day("2026-10-17");
        let ping = due(&Sent::default(), today, "0.9.10", "windows", "en").unwrap();
        let mut sent = Sent::default();
        sent.record(&ping, today);
        sent.save(&path).unwrap();
        assert_eq!(Sent::load(&path), after("2026-10-17", "2026-10"));
        // A damaged file counts as nothing sent, and a damaged month is replaced.
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Sent::load(&path), Sent::default());
        let odd = Sent {
            last: Some(day("2026-10-01")),
            since: Some("2026-13".into()),
        };
        assert_eq!(
            due(&odd, today, "0.9.10", "windows", "en").unwrap().since,
            "2026-10"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_json_names_nothing_else() {
        let ping = due(
            &Sent::default(),
            day("2026-10-17"),
            "0.9.10",
            "windows",
            "en",
        )
        .unwrap();
        let json = serde_json::to_value(&ping).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["lang", "month", "new", "os", "since", "version", "week"]
        );
    }
}
