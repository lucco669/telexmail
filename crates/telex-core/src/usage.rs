//! What the mail takes and how it moves: the Usage page in Settings, and the small meter
//! in the status bar.
//!
//! Per account: the server's storage quota when the server reports one (IMAP QUOTA, read
//! at each sync), how much each folder takes, mail received and sent per day over the last
//! [`FLOW_DAYS`] days, the busiest senders, the largest messages, what still waits for the
//! server, and how the last sync went. All of it comes from the local mail base, so it
//! shows offline; the counts cover the mail Telex has downloaded.

use chrono::{Duration, NaiveDate, NaiveDateTime};
use rusqlite::params_from_iter;
use rusqlite::types::Value;
use serde::Serialize;

use crate::model::{Account, FolderKind, MessageSummary};
use crate::store::{summary_from_row, MailStore, StoreError, SUMMARY_COLUMNS};

/// How many days the flow covers, today included.
pub const FLOW_DAYS: usize = 30;
/// How many senders and large messages the page lists.
const TOP: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub accounts: Vec<AccountUsage>,
    /// The mail base file, in bytes, and how much of it is free pages SQLite fills
    /// before the file grows again.
    pub base_bytes: u64,
    pub free_bytes: u64,
    /// The first day of every account's `flow`.
    pub since: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountUsage {
    pub account: String,
    /// The server's storage quota, when it reports one.
    pub quota: Option<Quota>,
    /// When a sync last worked, and why the last one failed (`None` when it worked).
    pub synced_at: Option<NaiveDateTime>,
    pub sync_error: Option<String>,
    /// Folders that hold mail, largest first.
    pub folders: Vec<FolderUsage>,
    /// One entry per day, oldest first, ending today.
    pub flow: Vec<DayFlow>,
    /// Who sent the most mail in those days (Spam left out).
    pub senders: Vec<SenderCount>,
    pub largest: Vec<MessageSummary>,
    /// Changes waiting to reach the server.
    pub queued: usize,
}

/// Storage on the server, in KiB as the server counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Quota {
    pub used_kb: u64,
    pub limit_kb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderUsage {
    pub folder: String,
    pub kind: FolderKind,
    pub count: usize,
    pub unread: usize,
    pub size_kb: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DayFlow {
    pub received: usize,
    pub sent: usize,
    pub spam: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SenderCount {
    pub name: String,
    pub email: String,
    pub count: usize,
}

/// Mail that came in: everything but Spam and what the user wrote.
fn is_received(kind: FolderKind) -> bool {
    !kind.is_outgoing() && kind != FolderKind::Spam && kind != FolderKind::Outbox
}

/// `?, ?, ?` for an IN list of `n` values.
fn marks(n: usize) -> String {
    vec!["?"; n].join(", ")
}

impl MailStore {
    /// Everything the Usage page shows, for the days up to `today`.
    pub fn usage(&self, today: NaiveDate) -> Result<Usage, StoreError> {
        let since = today - Duration::days(FLOW_DAYS as i64 - 1);
        let accounts = self
            .accounts()
            .iter()
            .map(|a| self.account_usage(a, since))
            .collect::<Result<_, _>>()?;
        let pragma = |name: &str| -> Result<u64, StoreError> {
            let v: i64 = self.db.pragma_query_value(None, name, |r| r.get(0))?;
            Ok(u64::try_from(v).unwrap_or_default())
        };
        let page = pragma("page_size")?;
        Ok(Usage {
            accounts,
            base_bytes: pragma("page_count")? * page,
            free_bytes: pragma("freelist_count")? * page,
            since,
        })
    }

    fn account_usage(
        &self,
        account: &Account,
        since: NaiveDate,
    ) -> Result<AccountUsage, StoreError> {
        let (used, limit, synced_at, sync_error): (Option<i64>, Option<i64>, _, _) =
            self.db.query_row(
                "SELECT quota_used_kb, quota_limit_kb, synced_at, sync_error FROM accounts WHERE id = ?1",
                [&account.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
        let quota = match (used, limit) {
            (Some(used), Some(limit)) if limit > 0 => Some(Quota {
                used_kb: u64::try_from(used).unwrap_or_default(),
                limit_kb: u64::try_from(limit).unwrap_or_default(),
            }),
            _ => None,
        };
        let queued = self.db.query_row(
            "SELECT COUNT(*) FROM pending_ops WHERE account = ?1",
            [&account.id],
            |r| r.get::<_, i64>(0),
        )?;
        let kinds: Vec<(String, FolderKind)> = account
            .folders
            .iter()
            .map(|f| (account.folder_key(&f.id), f.kind))
            .collect();
        let kind_of = |key: &str| kinds.iter().find(|(k, _)| k == key).map(|(_, kind)| *kind);
        let keys = |keep: &dyn Fn(FolderKind) -> bool| -> Vec<Value> {
            kinds
                .iter()
                .filter(|(_, kind)| keep(*kind))
                .map(|(k, _)| Value::Text(k.clone()))
                .collect()
        };
        let all = keys(&|_| true);
        let received = keys(&is_received);
        let mut usage = AccountUsage {
            account: account.id.clone(),
            quota,
            synced_at,
            sync_error,
            folders: Vec::new(),
            flow: vec![DayFlow::default(); FLOW_DAYS],
            senders: Vec::new(),
            largest: Vec::new(),
            queued: usize::try_from(queued).unwrap_or_default(),
        };
        if all.is_empty() {
            return Ok(usage);
        }

        let mut stmt = self.db.prepare(&format!(
            "SELECT folder, COUNT(*), SUM(unread), SUM(size_kb) FROM messages
             WHERE folder IN ({}) GROUP BY folder",
            marks(all.len())
        ))?;
        usage.folders = stmt
            .query_map(params_from_iter(&all), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(folder, count, unread, size)| {
                Some(FolderUsage {
                    kind: kind_of(&folder)?,
                    folder,
                    count: usize::try_from(count).unwrap_or_default(),
                    unread: usize::try_from(unread).unwrap_or_default(),
                    size_kb: u64::try_from(size).unwrap_or_default(),
                })
            })
            .collect();
        usage
            .folders
            .sort_by(|a, b| b.size_kb.cmp(&a.size_kb).then(a.folder.cmp(&b.folder)));

        // Dates are stored as "2026-10-10 06:12:00", so the day is the first ten characters.
        let first_day = since.format("%Y-%m-%d").to_string();
        let mut args = all.clone();
        args.push(Value::Text(first_day.clone()));
        let mut stmt = self.db.prepare(&format!(
            "SELECT substr(date, 1, 10), folder, COUNT(*) FROM messages
             WHERE folder IN ({}) AND date >= ? GROUP BY 1, 2",
            marks(all.len())
        ))?;
        let rows = stmt
            .query_map(params_from_iter(&args), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (day, folder, count) in rows {
            let Ok(day) = NaiveDate::parse_from_str(&day, "%Y-%m-%d") else {
                continue;
            };
            let Some(slot) = usize::try_from((day - since).num_days())
                .ok()
                .and_then(|i| usage.flow.get_mut(i))
            else {
                continue; // a date in the future
            };
            let count = usize::try_from(count).unwrap_or_default();
            match kind_of(&folder) {
                Some(FolderKind::Sent) => slot.sent += count,
                Some(FolderKind::Spam) => slot.spam += count,
                Some(kind) if is_received(kind) => slot.received += count,
                _ => {}
            }
        }

        if !received.is_empty() {
            let mut args = received.clone();
            args.push(Value::Text(first_day));
            let mut stmt = self.db.prepare(&format!(
                "SELECT MAX(from_name), lower(from_email), COUNT(*) FROM messages
                 WHERE folder IN ({}) AND date >= ? AND from_email <> ''
                 GROUP BY 2 ORDER BY 3 DESC, 2 LIMIT {TOP}",
                marks(received.len())
            ))?;
            usage.senders = stmt
                .query_map(params_from_iter(&args), |r| {
                    Ok(SenderCount {
                        name: r.get(0)?,
                        email: r.get(1)?,
                        count: usize::try_from(r.get::<_, i64>(2)?).unwrap_or_default(),
                    })
                })?
                .collect::<Result<_, _>>()?;
        }

        let mut stmt = self.db.prepare(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM messages WHERE folder IN ({})
             ORDER BY size_kb DESC, id DESC LIMIT {TOP}",
            marks(all.len())
        ))?;
        usage.largest = stmt
            .query_map(params_from_iter(&all), summary_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(usage)
    }

    /// Keep what the server said about its storage at the last sync: `None` when it
    /// doesn't report a quota.
    pub fn set_quota(&mut self, account: &str, quota: Option<Quota>) -> Result<(), StoreError> {
        let (used, limit) = match quota {
            Some(q) => (Some(q.used_kb as i64), Some(q.limit_kb as i64)),
            None => (None, None),
        };
        self.db.execute(
            "UPDATE accounts SET quota_used_kb = ?2, quota_limit_kb = ?3 WHERE id = ?1",
            rusqlite::params![account, used, limit],
        )?;
        Ok(())
    }

    /// How a sync of the account ended: at `at`, and why it failed (`None` when it worked).
    pub fn record_sync(
        &mut self,
        account: &str,
        at: NaiveDateTime,
        error: Option<&str>,
    ) -> Result<(), StoreError> {
        match error {
            None => self.db.execute(
                "UPDATE accounts SET synced_at = ?2, sync_error = NULL WHERE id = ?1",
                rusqlite::params![account, at],
            )?,
            Some(e) => self.db.execute(
                "UPDATE accounts SET sync_error = ?2 WHERE id = ?1",
                rusqlite::params![account, e],
            )?,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_store;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn usage_counts_folders_flow_and_senders() {
        let store = sample_store();
        let today = day(2026, 10, 10);
        let usage = store.usage(today).unwrap();
        assert_eq!(usage.since, day(2026, 9, 11));
        assert!(usage.base_bytes > 0);
        let firma = usage
            .accounts
            .iter()
            .find(|a| a.account == "firma")
            .unwrap();

        let inbox = firma
            .folders
            .iter()
            .find(|f| f.folder == "firma/inbox")
            .unwrap();
        assert_eq!(inbox.count, 12);
        assert!(inbox.unread > 0 && inbox.size_kb > 0);
        assert!(
            firma
                .folders
                .windows(2)
                .all(|w| w[0].size_kb >= w[1].size_kb),
            "largest first"
        );
        assert!(
            firma.folders.iter().all(|f| f.count > 0),
            "no empty folders"
        );

        assert_eq!(firma.flow.len(), FLOW_DAYS);
        let sent: usize = firma.flow.iter().map(|d| d.sent).sum();
        assert_eq!(
            sent, 2,
            "both of Firma's sent messages are from the last 30 days"
        );
        assert_eq!(firma.flow[FLOW_DAYS - 3].sent, 1, "one on 8 October");
        let received: usize = firma.flow.iter().map(|d| d.received).sum();
        let spam: usize = firma.flow.iter().map(|d| d.spam).sum();
        assert!(received >= 15, "{received}");
        assert_eq!(spam, 2);

        assert!(!firma.senders.is_empty() && firma.senders.len() <= TOP);
        assert!(firma.senders.windows(2).all(|w| w[0].count >= w[1].count));
        assert!(firma
            .senders
            .iter()
            .all(|s| s.email == s.email.to_lowercase()));

        assert_eq!(firma.largest.len(), TOP);
        assert!(firma
            .largest
            .windows(2)
            .all(|w| w[0].size_kb >= w[1].size_kb));
        assert!(firma.quota.is_none() && firma.synced_at.is_none() && firma.queued == 0);
    }

    #[test]
    fn quota_and_the_last_sync_are_kept() {
        let mut store = sample_store();
        let at = day(2026, 10, 10).and_hms_opt(9, 30, 0).unwrap();
        store
            .set_quota(
                "firma",
                Some(Quota {
                    used_kb: 512,
                    limit_kb: 10_240,
                }),
            )
            .unwrap();
        store.record_sync("firma", at, None).unwrap();
        store
            .record_sync(
                "firma",
                at + Duration::minutes(10),
                Some("can't reach mail.x.pl"),
            )
            .unwrap();
        let usage = store.usage(at.date()).unwrap();
        let firma = &usage.accounts[0];
        assert_eq!(
            firma.quota,
            Some(Quota {
                used_kb: 512,
                limit_kb: 10_240
            })
        );
        assert_eq!(firma.synced_at, Some(at), "the last sync that worked");
        assert_eq!(firma.sync_error.as_deref(), Some("can't reach mail.x.pl"));

        store.record_sync("firma", at, None).unwrap();
        store.set_quota("firma", None).unwrap();
        let firma = &store.usage(at.date()).unwrap().accounts[0];
        assert!(firma.sync_error.is_none() && firma.quota.is_none());
    }
}
