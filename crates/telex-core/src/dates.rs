//! Dates and times, with jiff (chrono, used before 0.9.24, is unmaintained: RUSTSEC-2026-0543).
//!
//! Telex works in this PC's local wall-clock time (`civil::DateTime`, no zone attached): the
//! date a message arrived, when it goes, a backup's time. An instant on the world's clock
//! (`Timestamp`) is used where one is needed: a server's INTERNALDATE, a certificate's validity.
//!
//! The mail base keeps a local date and time as text, as chrono wrote it: "2026-10-10 06:12:00",
//! with a fraction of a second only when there is one. Old rows and new ones must sort and compare
//! alike as text, so dates go into SQL through [`Sql`] (and come out through it), never as jiff's
//! own "2026-10-10T06:12:00".

use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, ZonedDateTime};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

/// This PC's wall clock now.
pub fn now() -> DateTime {
    ZonedDateTime::now().datetime()
}

/// Today on this PC's wall clock.
pub fn today() -> Date {
    ZonedDateTime::now().date()
}

/// An instant as this PC's wall clock shows it.
pub fn local(at: Timestamp) -> DateTime {
    at.to_zoned_datetime(TimeZone::system()).datetime()
}

/// The instant a wall-clock time of this PC stands for. In the hour that comes twice in autumn
/// the earlier one; in the hour skipped in spring, as far past the gap as it was into it.
pub fn instant(at: DateTime) -> Option<Timestamp> {
    TimeZone::system().to_timestamp(at).ok()
}

/// The instant a wall time on UTC's clock stands for (calendars keep their times in UTC).
pub fn utc_instant(utc: DateTime) -> Timestamp {
    TimeZone::UTC
        .to_timestamp(utc)
        .unwrap_or(Timestamp::UNIX_EPOCH)
}

/// Seconds since 1970 (UTC) as this PC's wall clock: a file's time, a stored Unix time.
pub fn from_unix(seconds: i64) -> Option<DateTime> {
    Timestamp::from_second(seconds).ok().map(local)
}

/// How long from `earlier` to `later` on the wall clock (negative when `later` is earlier).
pub fn between(earlier: DateTime, later: DateTime) -> SignedDuration {
    later.duration_since(earlier)
}

/// A whole number of days, for arithmetic on wall-clock times.
pub const fn days(n: i64) -> SignedDuration {
    SignedDuration::from_hours(n.saturating_mul(24))
}

/// The mail base's text for a wall-clock time: "2026-10-10 06:12:00" ("…:00.25" with a fraction).
pub fn sql_text(at: DateTime) -> String {
    at.strftime("%Y-%m-%d %H:%M:%S%.f").to_string()
}

/// Reads "2026-10-10 06:12:00" (the mail base's own) and "2026-10-10T06:12:00", with or without
/// a fraction of a second.
pub fn parse_sql(text: &str) -> Option<DateTime> {
    let text = text.trim();
    let mut t = text.to_string();
    if t.len() > 10 && t.as_bytes()[10] == b' ' {
        t.replace_range(10..11, "T");
    }
    t.parse().ok()
}

/// A wall-clock time as the mail base keeps it, for SQL parameters and columns:
/// `params![Sql(date)]`, `row.get::<_, Sql>(0)?.0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sql(pub DateTime);

impl ToSql for Sql {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(sql_text(self.0)))
    }
}

impl FromSql for Sql {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        parse_sql(text)
            .map(Sql)
            .ok_or_else(|| FromSqlError::Other(format!("not a date and time: {text}").into()))
    }
}

/// A wall-clock time from a row's column.
pub fn get(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<DateTime> {
    Ok(row.get::<_, Sql>(column)?.0)
}

/// A wall-clock time from a row's column that may be NULL.
pub fn get_opt(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<Option<DateTime>> {
    Ok(row.get::<_, Option<Sql>>(column)?.map(|s| s.0))
}

/// A date from a row's column that may be NULL.
pub fn get_date_opt(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<Option<Date>> {
    Ok(row.get::<_, Option<SqlDate>>(column)?.map(|s| s.0))
}

/// A date as the mail base keeps it: "2026-10-10".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SqlDate(pub Date);

impl ToSql for SqlDate {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.to_string()))
    }
}

impl FromSql for SqlDate {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        text.trim()
            .get(..10)
            .and_then(|d| d.parse().ok())
            .map(SqlDate)
            .ok_or_else(|| FromSqlError::Other(format!("not a date: {text}").into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn imap_and_mbox_dates_read_as_they_did_with_chrono() {
        // INTERNALDATE (RFC 3501) as the vendored async-imap reads it; the day may be padded.
        let internal = |text: &str| {
            jiff::fmt::strtime::parse("%d-%b-%Y %H:%M:%S %z", text.trim_start())
                .and_then(|t| t.to_timestamp())
                .unwrap()
        };
        let nine = utc_instant(date(2026, 10, 9).at(12, 30, 0, 0));
        assert_eq!(internal("09-Oct-2026 14:30:00 +0200"), nine);
        assert_eq!(internal(" 9-Oct-2026 14:30:00 +0200"), nine);
        // An mbox "From " line's date, its words joined by single spaces.
        let from = |text: &str| DateTime::strptime("%a %b %e %H:%M:%S %Y", text).unwrap();
        assert_eq!(
            from("Sat Oct 10 22:35:00 2026"),
            date(2026, 10, 10).at(22, 35, 0, 0)
        );
        assert_eq!(
            from("Thu Oct 1 09:00:00 2026"),
            date(2026, 10, 1).at(9, 0, 0, 0)
        );
    }

    #[test]
    fn the_mail_base_keeps_chronos_text() {
        let at = date(2026, 10, 10).at(6, 12, 0, 0);
        assert_eq!(sql_text(at), "2026-10-10 06:12:00");
        assert_eq!(
            sql_text(date(2026, 10, 10).at(6, 12, 0, 250_000_000)),
            "2026-10-10 06:12:00.25"
        );
        assert_eq!(parse_sql("2026-10-10 06:12:00"), Some(at));
        assert_eq!(parse_sql("2026-10-10T06:12:00"), Some(at));
        assert_eq!(
            parse_sql("2026-10-10 06:12:00.123456"),
            Some(date(2026, 10, 10).at(6, 12, 0, 123_456_000))
        );
        assert_eq!(parse_sql("10.10.2026"), None);
    }

    #[test]
    fn dates_go_through_sqlite_as_before() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE t (d TEXT, x TEXT)").unwrap();
        let at = date(2026, 3, 29).at(2, 30, 0, 0);
        db.execute(
            "INSERT INTO t VALUES (?1, ?2)",
            rusqlite::params![Sql(at), SqlDate(at.date())],
        )
        .unwrap();
        let (text, back, day): (String, Sql, SqlDate) = db
            .query_row("SELECT d, d, x FROM t", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(text, "2026-03-29 02:30:00");
        assert_eq!(back.0, at);
        assert_eq!(day.0, at.date());
        // SQLite's own date functions read it, as they read chrono's.
        let unix: i64 = db
            .query_row("SELECT strftime('%s', d) FROM t", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(unix, 1_774_751_400);
    }
}
