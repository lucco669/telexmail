//! The time zones of calendar invitations.
//!
//! A TZID is first looked up by name in the IANA database (jiff's, compiled in): IANA names
//! as Google and Apple write them (also behind a path such as `/mozilla.org/…/Europe/Warsaw`),
//! and the Windows names Outlook writes (`Central European Standard Time`), through CLDR's
//! table. Anything else (Outlook's "Customized Time Zone", "(UTC+01:00) Sarajevo, Skopje,
//! Warsaw, Zagreb") comes from the calendar's own VTIMEZONE: its STANDARD and DAYLIGHT
//! observances, each with a start, offsets and a yearly rule or dates.

use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::SignedDuration;

use super::{Component, Freq, Moment, Prop, Rule};

/// Observances of one VTIMEZONE read at most.
const MAX_OBSERVANCES: usize = 100;
/// RDATEs of one observance read at most.
const MAX_RDATES: usize = 400;

/// A zone a calendar's times are written in.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    name: String,
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Iana(TimeZone),
    Rules(Vec<Observance>),
}

/// One STANDARD or DAYLIGHT part of a VTIMEZONE.
#[derive(Debug, Clone, PartialEq)]
struct Observance {
    /// Seconds east of UTC before and after it starts.
    from: i32,
    to: i32,
    /// Its first start, a wall time in `from`.
    start: DateTime,
    /// A yearly rule (BYMONTH and BYDAY), when it repeats.
    rule: Option<Rule>,
    /// Further starts, wall times in `from`.
    rdates: Vec<DateTime>,
}

impl Zone {
    /// For showing: "Pacific Standard Time", "America/New York".
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Seconds east of UTC at a UTC time.
    pub fn offset_at(&self, utc: DateTime) -> i32 {
        match &self.kind {
            Kind::Iana(tz) => tz.to_offset(crate::dates::utc_instant(utc)).seconds(),
            Kind::Rules(observances) => rules_offset(observances, utc),
        }
    }

    /// The UTC time of a wall time in this zone. A wall time in the gap of a change to summer
    /// time is read with the offset before the change (so 02:30 becomes 03:30); one that
    /// comes twice is the first of the two (RFC 5545 3.3.5).
    pub fn to_utc(&self, local: DateTime) -> DateTime {
        let day = crate::dates::days(1);
        let before = self.offset_at(local - day);
        let after = self.offset_at(local + day);
        let valid = [before, after]
            .into_iter()
            .filter(|o| self.offset_at(local - SignedDuration::from_secs(i64::from(*o))) == *o)
            .max();
        local - SignedDuration::from_secs(i64::from(valid.unwrap_or(before)))
    }
}

/// The zone of a TZID: by name, else from the VTIMEZONE of that TZID.
pub fn find(tzid: &str, vtimezones: &[Component]) -> Option<Zone> {
    let tzid = tzid.trim().trim_matches('"').trim();
    if let Some(tz) = iana(tzid) {
        return Some(Zone {
            name: display(tzid, &tz),
            kind: Kind::Iana(tz),
        });
    }
    let vt = vtimezones.iter().find(|c| {
        c.value("TZID")
            .is_some_and(|t| t.trim().trim_matches('"').trim() == tzid)
    })?;
    // Apple and Mozilla name the IANA zone inside.
    if let Some(tz) = vt.value("X-LIC-LOCATION").and_then(iana) {
        return Some(Zone {
            name: display(tzid, &tz),
            kind: Kind::Iana(tz),
        });
    }
    let observances = observances(vt);
    (!observances.is_empty()).then(|| Zone {
        name: tzid.chars().take(100).collect(),
        kind: Kind::Rules(observances),
    })
}

/// The IANA zone a TZID names, directly, behind a path, or as Windows names it.
pub fn iana(tzid: &str) -> Option<TimeZone> {
    let t = tzid.trim().trim_matches('"').trim();
    if t.is_empty() || t.len() > 200 {
        return None;
    }
    if let Ok(tz) = TimeZone::get(t) {
        return Some(tz);
    }
    let parts: Vec<&str> = t.split('/').filter(|p| !p.is_empty()).collect();
    for n in [3, 2] {
        if parts.len() > n {
            if let Ok(tz) = TimeZone::get(&parts[parts.len() - n..].join("/")) {
                return Some(tz);
            }
        }
    }
    windows(t).and_then(|name| TimeZone::get(name).ok())
}

/// How a zone is named on the card: Windows names as they are, IANA ones without their path
/// and underscores.
fn display(tzid: &str, tz: &TimeZone) -> String {
    if windows(tzid).is_some() {
        tzid.to_owned()
    } else {
        tz.iana_name().unwrap_or(tzid).replace('_', " ")
    }
}

/// "+0100", "-0500", "+053000" → seconds.
fn offset(v: &str) -> Option<i32> {
    let v = v.trim();
    if !v.is_ascii() || v.len() < 5 {
        return None;
    }
    let sign = match &v[..1] {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let digits = &v[1..];
    let h: i32 = digits.get(0..2)?.parse().ok()?;
    let m: i32 = digits.get(2..4)?.parse().ok()?;
    let s: i32 = match digits.get(4..6) {
        Some(s) => s.parse().ok()?,
        None => 0,
    };
    (h < 24 && m < 60 && s < 60).then_some(sign * (h * 3600 + m * 60 + s))
}

fn wall(m: Moment) -> DateTime {
    match m {
        Moment::Utc(at) | Moment::Zoned(at, _) | Moment::Floating(at) => at,
        Moment::Date(d) => d.at(0, 0, 0, 0),
    }
}

fn observances(vt: &Component) -> Vec<Observance> {
    vt.children
        .iter()
        .filter(|c| c.name == "STANDARD" || c.name == "DAYLIGHT")
        .take(MAX_OBSERVANCES)
        .filter_map(|c| {
            let from = offset(c.value("TZOFFSETFROM")?)?;
            let to = offset(c.value("TZOFFSETTO")?)?;
            let start = wall(Moment::read(c.prop("DTSTART")?)?);
            let rule = c
                .value("RRULE")
                .and_then(Rule::read)
                .filter(|r| r.freq == Freq::Yearly);
            let rdates = c
                .props("RDATE")
                .flat_map(|p| {
                    p.value.split(',').take(MAX_RDATES).filter_map(|v| {
                        Moment::read(&Prop {
                            value: v.to_owned(),
                            params: p.params.clone(),
                            ..Prop::default()
                        })
                    })
                })
                .take(MAX_RDATES)
                .map(wall)
                .collect();
            Some(Observance {
                from,
                to,
                start,
                rule,
                rdates,
            })
        })
        .collect()
}

/// When an observance's yearly rule starts it in year `y`, as a wall time in its `from`.
fn onset_in(o: &Observance, r: &Rule, y: i16) -> Option<DateTime> {
    let first = o.start.year();
    if y < first {
        return None;
    }
    if r.count
        .is_some_and(|c| i64::from(y - first) >= i64::from(c))
    {
        return None;
    }
    let month = r
        .by_month
        .first()
        .map_or(o.start.month(), |m| i8::try_from(*m).unwrap_or(0));
    let date = if r.by_day.is_empty() {
        let d = match r.by_month_day.first() {
            Some(d) if *d > 0 => *d,
            Some(_) => return None,
            None => o.start.day(),
        };
        Date::new(y, month, d).ok()?
    } else {
        let (n, weekday) = r.by_day[0];
        let days: Vec<Date> = (1..=31)
            .filter_map(|d| Date::new(y, month, d).ok())
            .filter(|d| d.weekday() == weekday)
            .filter(|d| r.by_month_day.is_empty() || r.by_month_day.contains(&d.day()))
            .collect();
        match n {
            0 => days.first().copied(),
            n if n > 0 => days.get(usize::try_from(n - 1).ok()?).copied(),
            n => days
                .len()
                .checked_sub(usize::try_from(-n).ok()?)
                .and_then(|i| days.get(i).copied()),
        }?
    };
    let at = date.to_datetime(o.start.time());
    if at < o.start {
        return None;
    }
    if let Some(until) = &r.until {
        let onset_utc = at - SignedDuration::from_secs(i64::from(o.from));
        let until_utc = match until {
            Moment::Utc(u) => *u,
            Moment::Date(d) => d.at(23, 59, 59, 0) - SignedDuration::from_secs(i64::from(o.from)),
            Moment::Floating(u) | Moment::Zoned(u, _) => {
                *u - SignedDuration::from_secs(i64::from(o.from))
            }
        };
        if onset_utc > until_utc {
            return None;
        }
    }
    Some(at)
}

/// The offset of the observance that started last before a UTC time.
fn rules_offset(observances: &[Observance], utc: DateTime) -> i32 {
    let mut best: Option<(DateTime, i32)> = None;
    for o in observances {
        let mut consider = |onset: DateTime| {
            let onset_utc = onset - SignedDuration::from_secs(i64::from(o.from));
            if onset_utc <= utc && best.is_none_or(|(b, _)| onset_utc > b) {
                best = Some((onset_utc, o.to));
            }
        };
        consider(o.start);
        for at in &o.rdates {
            consider(*at);
        }
        if let Some(r) = &o.rule {
            for y in (utc.year() - 1)..=utc.year() {
                if let Some(at) = onset_in(o, r, y) {
                    consider(at);
                }
            }
        }
    }
    best.map(|(_, to)| to).unwrap_or_else(|| {
        observances
            .iter()
            .min_by_key(|o| o.start)
            .map_or(0, |o| o.from)
    })
}

/// Windows' zone names and the IANA zones CLDR gives them (windowsZones.xml, territory 001).
const WINDOWS: &[(&str, &str)] = &[
    ("Dateline Standard Time", "Etc/GMT+12"),
    ("UTC-11", "Etc/GMT+11"),
    ("Aleutian Standard Time", "America/Adak"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Marquesas Standard Time", "Pacific/Marquesas"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("UTC-09", "Etc/GMT+9"),
    ("Pacific Standard Time (Mexico)", "America/Tijuana"),
    ("UTC-08", "Etc/GMT+8"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Mountain Standard Time (Mexico)", "America/Mazatlan"),
    ("Mountain Standard Time", "America/Denver"),
    ("Yukon Standard Time", "America/Whitehorse"),
    ("Central America Standard Time", "America/Guatemala"),
    ("Central Standard Time", "America/Chicago"),
    ("Easter Island Standard Time", "Pacific/Easter"),
    ("Central Standard Time (Mexico)", "America/Mexico_City"),
    ("Canada Central Standard Time", "America/Regina"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("Eastern Standard Time (Mexico)", "America/Cancun"),
    ("Eastern Standard Time", "America/New_York"),
    ("Haiti Standard Time", "America/Port-au-Prince"),
    ("Cuba Standard Time", "America/Havana"),
    ("US Eastern Standard Time", "America/Indiana/Indianapolis"),
    ("Turks And Caicos Standard Time", "America/Grand_Turk"),
    ("Paraguay Standard Time", "America/Asuncion"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Venezuela Standard Time", "America/Caracas"),
    ("Central Brazilian Standard Time", "America/Cuiaba"),
    ("SA Western Standard Time", "America/La_Paz"),
    ("Pacific SA Standard Time", "America/Santiago"),
    ("Newfoundland Standard Time", "America/St_Johns"),
    ("Tocantins Standard Time", "America/Araguaina"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("SA Eastern Standard Time", "America/Cayenne"),
    ("Argentina Standard Time", "America/Argentina/Buenos_Aires"),
    ("Greenland Standard Time", "America/Nuuk"),
    ("Montevideo Standard Time", "America/Montevideo"),
    ("Magallanes Standard Time", "America/Punta_Arenas"),
    ("Saint Pierre Standard Time", "America/Miquelon"),
    ("Bahia Standard Time", "America/Bahia"),
    ("UTC-02", "Etc/GMT+2"),
    ("Mid-Atlantic Standard Time", "Etc/GMT+2"),
    ("Azores Standard Time", "Atlantic/Azores"),
    ("Cape Verde Standard Time", "Atlantic/Cape_Verde"),
    ("UTC", "Etc/UTC"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("Sao Tome Standard Time", "Africa/Sao_Tome"),
    ("Morocco Standard Time", "Africa/Casablanca"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Romance Standard Time", "Europe/Paris"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("W. Central Africa Standard Time", "Africa/Lagos"),
    ("Jordan Standard Time", "Asia/Amman"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("Middle East Standard Time", "Asia/Beirut"),
    ("Egypt Standard Time", "Africa/Cairo"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("Syria Standard Time", "Asia/Damascus"),
    ("West Bank Standard Time", "Asia/Hebron"),
    ("South Africa Standard Time", "Africa/Johannesburg"),
    ("FLE Standard Time", "Europe/Kyiv"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("South Sudan Standard Time", "Africa/Juba"),
    ("Kaliningrad Standard Time", "Europe/Kaliningrad"),
    ("Sudan Standard Time", "Africa/Khartoum"),
    ("Libya Standard Time", "Africa/Tripoli"),
    ("Namibia Standard Time", "Africa/Windhoek"),
    ("Arabic Standard Time", "Asia/Baghdad"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Arab Standard Time", "Asia/Riyadh"),
    ("Belarus Standard Time", "Europe/Minsk"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("E. Africa Standard Time", "Africa/Nairobi"),
    ("Volgograd Standard Time", "Europe/Volgograd"),
    ("Iran Standard Time", "Asia/Tehran"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("Astrakhan Standard Time", "Europe/Astrakhan"),
    ("Azerbaijan Standard Time", "Asia/Baku"),
    ("Russia Time Zone 3", "Europe/Samara"),
    ("Mauritius Standard Time", "Indian/Mauritius"),
    ("Saratov Standard Time", "Europe/Saratov"),
    ("Georgian Standard Time", "Asia/Tbilisi"),
    ("Caucasus Standard Time", "Asia/Yerevan"),
    ("Afghanistan Standard Time", "Asia/Kabul"),
    ("West Asia Standard Time", "Asia/Tashkent"),
    ("Ekaterinburg Standard Time", "Asia/Yekaterinburg"),
    ("Pakistan Standard Time", "Asia/Karachi"),
    ("Qyzylorda Standard Time", "Asia/Qyzylorda"),
    ("India Standard Time", "Asia/Kolkata"),
    ("Sri Lanka Standard Time", "Asia/Colombo"),
    ("Nepal Standard Time", "Asia/Kathmandu"),
    ("Central Asia Standard Time", "Asia/Bishkek"),
    ("Bangladesh Standard Time", "Asia/Dhaka"),
    ("Omsk Standard Time", "Asia/Omsk"),
    ("Myanmar Standard Time", "Asia/Yangon"),
    ("SE Asia Standard Time", "Asia/Bangkok"),
    ("Altai Standard Time", "Asia/Barnaul"),
    ("W. Mongolia Standard Time", "Asia/Hovd"),
    ("North Asia Standard Time", "Asia/Krasnoyarsk"),
    ("N. Central Asia Standard Time", "Asia/Novosibirsk"),
    ("Tomsk Standard Time", "Asia/Tomsk"),
    ("China Standard Time", "Asia/Shanghai"),
    ("North Asia East Standard Time", "Asia/Irkutsk"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("W. Australia Standard Time", "Australia/Perth"),
    ("Taipei Standard Time", "Asia/Taipei"),
    ("Ulaanbaatar Standard Time", "Asia/Ulaanbaatar"),
    ("Aus Central W. Standard Time", "Australia/Eucla"),
    ("Transbaikal Standard Time", "Asia/Chita"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("North Korea Standard Time", "Asia/Pyongyang"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("Yakutsk Standard Time", "Asia/Yakutsk"),
    ("Cen. Australia Standard Time", "Australia/Adelaide"),
    ("AUS Central Standard Time", "Australia/Darwin"),
    ("E. Australia Standard Time", "Australia/Brisbane"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("West Pacific Standard Time", "Pacific/Port_Moresby"),
    ("Tasmania Standard Time", "Australia/Hobart"),
    ("Vladivostok Standard Time", "Asia/Vladivostok"),
    ("Lord Howe Standard Time", "Australia/Lord_Howe"),
    ("Bougainville Standard Time", "Pacific/Bougainville"),
    ("Russia Time Zone 10", "Asia/Srednekolymsk"),
    ("Magadan Standard Time", "Asia/Magadan"),
    ("Norfolk Standard Time", "Pacific/Norfolk"),
    ("Sakhalin Standard Time", "Asia/Sakhalin"),
    ("Central Pacific Standard Time", "Pacific/Guadalcanal"),
    ("Russia Time Zone 11", "Asia/Kamchatka"),
    ("Kamchatka Standard Time", "Asia/Kamchatka"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
    ("UTC+12", "Etc/GMT-12"),
    ("Fiji Standard Time", "Pacific/Fiji"),
    ("Chatham Islands Standard Time", "Pacific/Chatham"),
    ("UTC+13", "Etc/GMT-13"),
    ("Tonga Standard Time", "Pacific/Tongatapu"),
    ("Samoa Standard Time", "Pacific/Apia"),
    ("Line Islands Standard Time", "Pacific/Kiritimati"),
];

/// The IANA name for a Windows zone name, case ignored.
fn windows(name: &str) -> Option<&'static str> {
    WINDOWS
        .iter()
        .find(|(w, _)| w.eq_ignore_ascii_case(name))
        .map(|(_, iana)| *iana)
}

#[cfg(test)]
pub(super) fn windows_names() -> impl Iterator<Item = (&'static str, &'static str)> {
    WINDOWS.iter().copied()
}
