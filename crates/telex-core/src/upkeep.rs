//! Work a schema upgrade leaves for after the window opens (schema 25, the `upkeep` table).
//!
//! Some upgrades read every stored message once: schema 18 looks for calendar invitations in
//! all mail, which took 10–15 s on a mail base of 2.85 GB, with the window waiting. Such a step
//! now only queues a job ([`queue`]); the app runs it a short step at a time
//! ([`MailStore::upkeep_step`]) with the store free for the window in between. A job works down
//! from the newest message, the mail most likely to be opened; each step is one transaction
//! that also moves the job's place, so a job cut short (Telex closed, the PC turned off) goes
//! on where it stopped at the next start. Mail stored meanwhile is handled as it is stored.

use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::store::{MailStore, StoreError};
use crate::MessageId;

/// Schema 18's invitations in the mail stored before it (`calendar::invites::index`).
pub const INVITES: &str = "invites";

/// What is left to do, in messages, for the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

/// Queue `job` for every message stored now. Nothing is queued when there is no mail.
pub(crate) fn queue(db: &Connection, job: &str) -> Result<(), StoreError> {
    db.execute(
        "INSERT OR REPLACE INTO upkeep (job, next, done, total)
         SELECT ?1, MAX(id) + 1, 0, COUNT(*) FROM message_content
         WHERE raw IS NOT NULL HAVING COUNT(*) > 0",
        [job],
    )?;
    Ok(())
}

/// Does `job` still have to reach this message? Then whoever needs its result now does the
/// message's part at once (an invitation opened before the job got to it).
pub(crate) fn waiting(db: &Connection, job: &str, id: MessageId) -> Result<bool, StoreError> {
    let next: Option<i64> = db
        .prepare_cached("SELECT next FROM upkeep WHERE job = ?1")?
        .query_row([job], |r| r.get(0))
        .optional()?;
    Ok(next.is_some_and(|next| (id as i64) < next))
}

/// One message's part of a job.
fn work(db: &Connection, job: &str, id: MessageId, raw: &[u8]) -> Result<(), StoreError> {
    match job {
        INVITES => crate::calendar::invites::index(db, id, Some(raw)),
        _ => Ok(()),
    }
}

impl MailStore {
    /// What upgrades left to do, or `None` when nothing is.
    pub fn upkeep_left(&self) -> Result<Option<Progress>, StoreError> {
        let (rows, done, total): (i64, i64, i64) = self.db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(done), 0), COALESCE(SUM(total), 0) FROM upkeep",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok((rows > 0).then(|| Progress {
            done: done.min(total) as u64,
            total: total as u64,
        }))
    }

    /// Work on what upgrades left for about `budget` (at least one message), then say what is
    /// still left: `None` when it is all done. The store is busy only for this step.
    pub fn upkeep_step(&mut self, budget: Duration) -> Result<Option<Progress>, StoreError> {
        let job: Option<(String, i64, i64)> = self
            .db
            .query_row(
                "SELECT job, next, done FROM upkeep ORDER BY job LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((job, mut next, mut done)) = job else {
            return Ok(None);
        };
        let deadline = Instant::now().checked_add(budget);
        let tx = self.db.transaction()?;
        let mut finished = true;
        {
            let mut stmt = tx.prepare(
                "SELECT id, raw FROM message_content
                 WHERE id < ?1 AND raw IS NOT NULL ORDER BY id DESC",
            )?;
            let mut rows = stmt.query([next])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                // The message as SQLite holds it, without a copy.
                let raw = row.get_ref(1)?.as_bytes().unwrap_or_default();
                work(&tx, &job, id as MessageId, raw)?;
                (next, done) = (id, done + 1);
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    finished = rows.next()?.is_none();
                    break;
                }
            }
        }
        if finished {
            tx.execute("DELETE FROM upkeep WHERE job = ?1", [&job])?;
        } else {
            tx.execute(
                "UPDATE upkeep SET next = ?1, done = ?2 WHERE job = ?3",
                params![next, done, job],
            )?;
        }
        tx.commit()?;
        self.upkeep_left()
    }
}
