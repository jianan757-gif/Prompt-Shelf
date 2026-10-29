use crate::store::Store;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageReference {
    pub id: String,
    pub asset_id: String,
    pub original_name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseDocument {
    pub id: String,
    pub base_revision: u64,
    pub title: String,
    pub prompt: String,
    pub project: String,
    pub tags: Vec<String>,
    pub source_url: String,
    pub result: Option<ImageReference>,
    pub references: Vec<ImageReference>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedCase {
    pub id: String,
    pub edit_revision: u64,
}
fn sql(error: rusqlite::Error) -> String {
    format!("sqlite: {error}")
}
fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub fn normalize_source(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.chars().any(char::is_control) {
        return Err("invalid_source_url".into());
    }
    let lower = value.to_ascii_lowercase();
    let candidate = if lower.starts_with("http://") || lower.starts_with("https://") {
        value.to_owned()
    } else if value.starts_with("//") {
        format!("https:{value}")
    } else {
        if let Some((prefix, rest)) = value.split_once(':') {
            let host_port = (prefix.contains('.') || prefix == "localhost")
                && rest.split('/').next().unwrap_or("").parse::<u16>().is_ok();
            if !host_port {
                return Err("source_url_requires_http_or_https".into());
            }
        }
        format!("https://{value}")
    };
    let url = tauri::Url::parse(&candidate).map_err(|_| "invalid_source_url")?;
    if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() {
        return Err("source_url_requires_http_or_https".into());
    }
    Ok(url.into())
}
impl Store {
    pub fn edit_document(&self, case_id: Option<&str>) -> Result<CaseDocument, String> {
        let Some(case_id) = case_id else {
            return Ok(CaseDocument {
                id: uuid(),
                base_revision: 0,
                title: String::new(),
                prompt: String::new(),
                project: String::new(),
                tags: vec![],
                source_url: String::new(),
                result: None,
                references: vec![],
            });
        };
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let mut document=connection.query_row("SELECT c.id,c.edit_revision,c.title,c.prompt,coalesce(p.name,''),c.source_url FROM cases c LEFT JOIN projects p ON p.id=c.project_id WHERE c.id=?1 AND c.deleted_at IS NULL",[case_id],|r|Ok(CaseDocument{id:r.get(0)?,base_revision:r.get::<_,i64>(1)? as u64,title:r.get(2)?,prompt:r.get(3)?,project:r.get(4)?,source_url:r.get(5)?,tags:vec![],result:None,references:vec![]})).optional().map_err(sql)?.ok_or("case_missing_or_deleted")?;
        let mut tags=connection.prepare("SELECT t.name FROM case_tags ct JOIN tags t ON t.id=ct.tag_id WHERE ct.case_id=?1 ORDER BY ct.position").map_err(sql)?;
        document.tags = tags
            .query_map([case_id], |r| r.get(0))
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        let mut images=connection.prepare("SELECT id,asset_id,original_name,role FROM case_assets WHERE case_id=?1 ORDER BY position").map_err(sql)?;
        for row in images
            .query_map([case_id], |r| {
                Ok((
                    ImageReference {
                        id: r.get(0)?,
                        asset_id: r.get(1)?,
                        original_name: r.get(2)?,
                    },
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(sql)?
        {
            let (reference, role) = row.map_err(sql)?;
            if role == "result" {
                document.result = Some(reference)
            } else {
                document.references.push(reference)
            }
        }
        Ok(document)
    }
    /// One metadata transaction. No file deletion or favorite/use update.
    pub fn commit_case(&self, document: &CaseDocument) -> Result<SavedCase, String> {
        self.commit_case_consuming(document, None)
    }
    pub(crate) fn commit_case_consuming(&self, document: &CaseDocument, draft_id: Option<&str>) -> Result<SavedCase, String> {
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        if document.title.trim().is_empty() {
            return Err("case_title_required".into());
        }
        if document.prompt.trim().is_empty() {
            return Err("case_prompt_required".into());
        }
        uuid::Uuid::parse_str(&document.id).map_err(|_| "invalid_case_id")?;
        let source = normalize_source(&document.source_url)?;
        let mut ids = HashSet::new();
        for reference in document.result.iter().chain(&document.references) {
            uuid::Uuid::parse_str(&reference.id).map_err(|_| "invalid_reference_id")?;
            if !ids.insert(&reference.id) {
                return Err("duplicate_reference_id".into());
            }
        }
        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        if let Some(id) = draft_id {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM drafts WHERE id=?1 AND coalesce(case_id,id)=?2 AND base_revision=?3)", params![id,document.id,i64::try_from(document.base_revision).map_err(|_|"revision_overflow")?], |r|r.get(0)).map_err(sql)?;
            if !valid { return Err("draft_ended_or_mismatched".into()); }
        }
        let current: Option<(i64, Option<i64>)> = tx
            .query_row(
                "SELECT edit_revision,deleted_at FROM cases WHERE id=?1",
                [&document.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?;
        match current {
            None if document.base_revision == 0 => {}
            Some((revision, None)) if revision as u64 == document.base_revision => {}
            _ => return Err("case_revision_conflict_reload_or_keep_editing".into()),
        }
        // All original files must already be complete before formal references can commit.
        for reference in document.result.iter().chain(&document.references) {
            let relative: String = tx
                .query_row(
                    "SELECT relative_path FROM assets WHERE id=?1",
                    [&reference.asset_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?
                .ok_or("asset_not_registered")?;
            self.paths().resolve(&relative, "assets")?;
        }
        let timestamp = now();
        let project = if document.project.trim().is_empty() {
            None
        } else {
            Some(case_catalog(
                &tx,
                "projects",
                document.project.trim(),
                timestamp,
            )?)
        };
        let revision = document
            .base_revision
            .checked_add(1)
            .ok_or("revision_overflow")?;
        let sql_revision = i64::try_from(revision).map_err(|_| "revision_overflow")?;
        if current.is_none() {
            tx.execute("INSERT INTO cases(id,title,prompt,source_url,project_id,created_at,updated_at,edit_revision) VALUES(?1,?2,?3,?4,?5,?6,?6,1)",params![document.id,document.title.trim(),document.prompt,source,project,timestamp]).map_err(sql)?;
        } else {
            tx.execute("UPDATE cases SET title=?1,prompt=?2,source_url=?3,project_id=?4,updated_at=?5,edit_revision=?6 WHERE id=?7",params![document.title.trim(),document.prompt,source,project,timestamp,sql_revision,document.id]).map_err(sql)?;
        }
        tx.execute("DELETE FROM case_tags WHERE case_id=?1", [&document.id])
            .map_err(sql)?;
        let mut names = HashSet::new();
        let mut position = 0;
        for name in &document.tags {
            let name = name.trim();
            if name.is_empty() || !names.insert(name.to_lowercase()) {
                continue;
            }
            let tag = case_catalog(&tx, "tags", name, timestamp)?;
            tx.execute(
                "INSERT INTO case_tags(case_id,tag_id,position) VALUES(?1,?2,?3)",
                params![document.id, tag, position],
            )
            .map_err(sql)?;
            position += 1;
        }
        tx.execute("DELETE FROM case_assets WHERE case_id=?1", [&document.id])
            .map_err(sql)?;
        for (role, images) in [
            ("result", document.result.iter().collect::<Vec<_>>()),
            ("reference", document.references.iter().collect()),
        ] {
            for (position, image) in images.into_iter().enumerate() {
                tx.execute("INSERT INTO case_assets(id,case_id,asset_id,role,position,original_name) VALUES(?1,?2,?3,?4,?5,?6)",params![image.id,document.id,image.asset_id,role,position as i64,image.original_name]).map_err(sql)?;
                tx.execute(
                    "UPDATE assets SET gc_state='live' WHERE id=?1",
                    [&image.asset_id],
                )
                .map_err(sql)?;
            }
        }
        if let Some(id) = draft_id { tx.execute("DELETE FROM drafts WHERE id=?1",[id]).map_err(sql)?; }
        crate::gc::mark_unreferenced(&tx)?;
        tx.commit().map_err(sql)?;
        Ok(SavedCase {
            id: document.id.clone(),
            edit_revision: revision,
        })
    }
    pub fn case_snapshot_at_revision(
        &self,
        id: &str,
        expected: u64,
    ) -> Result<crate::data::Library, String> {
        let snapshot = self.case_snapshot(id)?;
        let case = snapshot.cases.first().ok_or("unknown_case")?;
        if case.edit_revision != expected {
            return Err("case_changed_refresh_and_retry".into());
        }
        Ok(snapshot)
    }
    /// A queue owns immutable paths after this returns, even if the source case is deleted.
    /// Keep the files until process exit: CF_HDROP can outlive queue completion/cancellation.
    pub fn output_snapshot_at_revision(&self, id: &str, expected: u64) -> Result<crate::data::Library, String> {
        let mut resources = self.resources.lock().map_err(|e| e.to_string())?;
        let snapshot = self.case_snapshot_at_revision(id, expected)?;
        resources.output.extend(snapshot.assets.iter().map(|a| a.id.clone()));
        Ok(snapshot)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditReceipt {
    pub session_id: String,
    pub document: CaseDocument,
    pub draft_id: Option<String>,
    pub save_seq: u64,
}
#[derive(Default)]
pub struct EditSessions {
    pub(crate) current: std::sync::Mutex<Option<(String, String, u64, Option<String>)>>,
    epoch: std::sync::atomic::AtomicU64,
}
impl EditSessions {
    /// Catalog names in an active editor are strings. Resolve that editor before mutating catalogs.
    /// The same lock excludes queued begin/stash/save; a completed change invalidates old begin epochs.
    pub fn while_idle<T>(&self,change:impl FnOnce()->Result<T,String>)->Result<T,String>{
        let slot=self.current.lock().map_err(|e|e.to_string())?;
        if slot.is_some(){return Err("resolve_current_editor_before_catalog_change".into())}
        let result=change()?;self.invalidate_epoch();Ok(result)
    }
    pub(crate) fn invalidate_epoch(&self) { self.epoch.fetch_add(1, std::sync::atomic::Ordering::AcqRel); }
    pub fn epoch(&self) -> u64 {
        self.epoch.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn begin_at(&self, document: CaseDocument, epoch: u64) -> Result<EditReceipt, String> {
        let mut slot = self.current.lock().map_err(|e| e.to_string())?;
        if self.epoch() != epoch {
            return Err("edit_window_changed".into());
        }
        if slot.is_some() {
            return Err("another_case_is_being_edited".into());
        }
        let session_id = uuid();
        *slot = Some((
            session_id.clone(),
            document.id.clone(),
            document.base_revision,
            None,
        ));
        Ok(EditReceipt {
            session_id,
            document,
            draft_id: None,
            save_seq: 0,
        })
    }
    pub fn check(&self, token: &str) -> Result<(), String> {
        if self
            .current
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .is_some_and(|current| current.0 == token)
        {
            Ok(())
        } else {
            Err("edit_session_expired".into())
        }
    }
    pub fn cancel(&self, token: &str) -> Result<(), String> {
        let mut slot = self.current.lock().map_err(|e| e.to_string())?;
        if !slot.as_ref().is_some_and(|current| current.0 == token) {
            return Err("edit_session_expired".into());
        }
        *slot = None;
        self.epoch.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(())
    }
    pub fn clear(&self) {
        if let Ok(mut slot) = self.current.lock() {
            *slot = None;
            self.epoch.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
    }
    pub fn save(
        &self,
        store: &Store,
        token: &str,
        document: &CaseDocument,
    ) -> Result<SavedCase, String> {
        let mut slot = self.current.lock().map_err(|e| e.to_string())?;
        if !slot.as_ref().is_some_and(|current| {
            current.0 == token && current.1 == document.id && current.2 == document.base_revision
        }) {
            return Err("edit_session_expired_or_mismatched".into());
        }
        let saved = store.commit_case_consuming(document, slot.as_ref().and_then(|s|s.3.as_deref()))?;
        *slot = None;
        self.epoch.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(saved)
    }
}
fn case_catalog(
    tx: &rusqlite::Connection,
    table: &str,
    name: &str,
    timestamp: i64,
) -> Result<String, String> {
    let key = name.trim().to_lowercase();
    if let Some(found) = tx
        .query_row(
            &format!("SELECT id FROM {table} WHERE name_key=?1"),
            [&key],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(sql)?
    {
        return Ok(found);
    }
    let id = uuid();
    tx.execute(
        &format!("INSERT INTO {table}(id,name,name_key,created_at) VALUES(?1,?2,?3,?4)"),
        params![id, name.trim(), key, timestamp],
    )
    .map_err(sql)?;
    Ok(id)
}
#[cfg(test)]
mod tests;
