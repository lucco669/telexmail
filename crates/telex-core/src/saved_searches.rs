//! Saved searches store a list query, never its results. Every open uses the current mail base.
use crate::{
    list::{every_kind, ListView},
    tr, MailStore, StoreError,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSearch {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub view: ListView,
}

impl MailStore {
    pub fn saved_searches(&self) -> Result<Vec<SavedSearch>, StoreError> {
        let mut statement = self.db.prepare(
            "SELECT id, name, view FROM saved_searches ORDER BY name COLLATE NOCASE, id",
        )?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        // One search a newer (or older) Telex wrote in a shape this one can't read is left
        // out; it mustn't take the others with it.
        let mut out = Vec::new();
        for row in rows {
            let (id, name, view) = row?;
            if let Ok(view) = serde_json::from_str(&view) {
                out.push(SavedSearch { id, name, view });
            }
        }
        Ok(out)
    }

    pub fn save_search(&mut self, search: &SavedSearch) -> Result<SavedSearch, StoreError> {
        let mut search = search.clone();
        search.name = search.name.trim().to_owned();
        search.view.filter = search.view.filter.trim().to_owned();
        if search.name.is_empty()
            || search.name.chars().count() > 80
            || search.name.chars().any(char::is_control)
        {
            return Err(StoreError::Invalid(tr!(
                "Give the saved search a name of 1–80 characters.",
                "Nadaj zapisanemu wyszukiwaniu nazwę od 1 do 80 znaków."
            )));
        }
        if search.view.filter.is_empty()
            || search.view.filter.len() > 4096
            || search.view.thread.is_some()
        {
            return Err(StoreError::Invalid(tr!(
                "Enter a search before saving it.",
                "Wpisz wyszukiwanie, zanim je zapiszesz."
            )));
        }
        if let Some(folder) = &search.view.folder {
            if every_kind(folder).is_none() && self.kind_of(folder).is_none() {
                return Err(StoreError::UnknownFolder(folder.clone()));
            }
        }
        // Temporary row retention/expansion belongs to the open view, not the saved query.
        search.view.keep.clear();
        search.view.expanded.clear();
        search.view.grouped = false;
        let duplicate: Option<i64> = self
            .db
            .query_row(
                "SELECT id FROM saved_searches WHERE name = ?1 AND id != ?2",
                params![search.name, search.id],
                |r| r.get(0),
            )
            .optional()?;
        if duplicate.is_some() {
            return Err(StoreError::Invalid(tr!(
                "A saved search already has that name.",
                "Zapisane wyszukiwanie ma już taką nazwę."
            )));
        }
        let view =
            serde_json::to_string(&search.view).map_err(|e| StoreError::Database(e.to_string()))?;
        if search.id == 0 {
            self.db.execute(
                "INSERT INTO saved_searches (name,view) VALUES (?1,?2)",
                params![search.name, view],
            )?;
            search.id = self.db.last_insert_rowid();
        } else if self.db.execute(
            "UPDATE saved_searches SET name=?2,view=?3 WHERE id=?1",
            params![search.id, search.name, view],
        )? == 0
        {
            return Err(StoreError::Invalid(tr!(
                "That saved search no longer exists.",
                "Tego zapisanego wyszukiwania już nie ma."
            )));
        }
        Ok(search)
    }

    pub fn delete_search(&mut self, id: i64) -> Result<(), StoreError> {
        self.db
            .execute("DELETE FROM saved_searches WHERE id=?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sample::sample_store, PageAt};
    #[test]
    fn saved_searches_persist_across_reopening_the_mail_base() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("telex-saved-search-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("telex.db");
        let saved = {
            let mut store = MailStore::open(&path).unwrap();
            store
                .save_search(&SavedSearch {
                    id: 0,
                    name: "Unread files".into(),
                    view: ListView {
                        filter: "is:unread has:file".into(),
                        keep: vec![123],
                        expanded: vec![123],
                        grouped: true,
                        ..ListView::default()
                    },
                })
                .unwrap()
        };
        {
            let store = MailStore::open(&path).unwrap();
            assert_eq!(
                store.saved_searches().unwrap().as_slice(),
                std::slice::from_ref(&saved)
            );
            assert!(saved.view.keep.is_empty() && saved.view.expanded.is_empty());
            assert!(!saved.view.grouped);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saved_queries_are_live_and_validate_scope() {
        let mut store = sample_store();
        let search = store
            .save_search(&SavedSearch {
                id: 0,
                name: "Invoices".into(),
                view: ListView {
                    folder: Some("firma/inbox".into()),
                    filter: "is:unread".into(),
                    ..ListView::default()
                },
            })
            .unwrap();
        assert_eq!(
            store.saved_searches().unwrap().as_slice(),
            std::slice::from_ref(&search)
        );
        let before = store.page(&search.view, PageAt::Row(0), 100).unwrap();
        assert!(before.total > 0);
        store.set_unread(before.rows[0].id, false).unwrap();
        assert_eq!(
            store.page(&search.view, PageAt::Row(0), 100).unwrap().total,
            before.total - 1
        );
        assert!(store
            .save_search(&SavedSearch {
                id: 0,
                ..search.clone()
            })
            .is_err());
        let mut bad = search.clone();
        bad.view.folder = Some("missing/folder".into());
        assert!(store.save_search(&bad).is_err());
        store.delete_search(search.id).unwrap();
        assert!(store.saved_searches().unwrap().is_empty());
    }

    #[test]
    fn one_unreadable_search_leaves_the_others() {
        let mut store = sample_store();
        let good = store
            .save_search(&SavedSearch {
                id: 0,
                name: "Unread".into(),
                view: ListView {
                    filter: "is:unread".into(),
                    ..ListView::default()
                },
            })
            .unwrap();
        store
            .db
            .execute(
                "INSERT INTO saved_searches (name, view) VALUES ('Broken', '{\"sort\": 42')",
                [],
            )
            .unwrap();
        assert_eq!(
            store.saved_searches().unwrap().as_slice(),
            std::slice::from_ref(&good)
        );
    }
}
