//! Folders' own templates (schema 15): what a new message written while a folder is open, and
//! a reply to mail in it, open with, in place of the identity's (`drafts::start`).

use rusqlite::{params, OptionalExtension};

use super::{MailStore, StoreError};
use crate::model::{split_folder_key, FolderTemplate};
use crate::tr;

impl MailStore {
    /// Every folder's templates, folder by folder as the tree lists them; those of folders
    /// that are gone are left out.
    pub fn folder_templates(&self) -> Result<Vec<FolderTemplate>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT account, folder, new_template, reply_template, reply_header
             FROM folder_templates",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FolderTemplate {
                    folder: format!("{}/{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                    new_template: r.get(2)?,
                    reply_template: r.get(3)?,
                    reply_header: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut found = Vec::new();
        for account in self.accounts() {
            for folder in &account.folders {
                let key = account.folder_key(&folder.id);
                if let Some(t) = rows.iter().find(|t| t.folder == key) {
                    found.push(t.clone());
                }
            }
        }
        Ok(found)
    }

    /// The templates of one folder, if it has any.
    pub fn folder_template(&self, folder: &str) -> Result<Option<FolderTemplate>, StoreError> {
        let Some((account, id)) = split_folder_key(folder) else {
            return Ok(None);
        };
        Ok(self
            .db
            .query_row(
                "SELECT new_template, reply_template, reply_header FROM folder_templates
                 WHERE account = ?1 AND folder = ?2",
                params![account, id],
                |r| {
                    Ok(FolderTemplate {
                        folder: folder.to_owned(),
                        new_template: r.get(0)?,
                        reply_template: r.get(1)?,
                        reply_header: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Keep a folder's templates; all of them empty takes them away.
    pub fn save_folder_template(&mut self, template: &FolderTemplate) -> Result<(), StoreError> {
        let exists = split_folder_key(&template.folder).and_then(|(account, id)| {
            let a = self.account(account).ok()?;
            a.folders
                .iter()
                .any(|f| f.id == id)
                .then_some((account, id))
        });
        let Some((account, id)) = exists else {
            return Err(StoreError::Invalid(tr!(
                "That folder no longer exists.",
                "Ten folder już nie istnieje."
            )));
        };
        if template.is_empty() {
            self.db.execute(
                "DELETE FROM folder_templates WHERE account = ?1 AND folder = ?2",
                params![account, id],
            )?;
            return Ok(());
        }
        self.db.execute(
            "INSERT INTO folder_templates (account, folder, new_template, reply_template, reply_header)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (account, folder) DO UPDATE SET new_template = excluded.new_template,
                 reply_template = excluded.reply_template, reply_header = excluded.reply_header",
            params![
                account,
                id,
                template.new_template,
                template.reply_template,
                template.reply_header
            ],
        )?;
        Ok(())
    }
}
