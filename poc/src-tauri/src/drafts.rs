//! Persistent drafts are separate from formal cases. Session lock always precedes Store lock.
use crate::{
    cases::{CaseDocument, EditReceipt, EditSessions, ImageReference},
    store::Store,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
fn sql(e: rusqlite::Error) -> String {
    format!("sqlite: {e}")
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftSummary {
    pub id: String,
    pub case_id: Option<String>,
    pub title: String,
    pub prompt: String,
    pub reference_count: i64,
    pub result_count: i64,
    pub updated_at: i64,
    pub target_status: String,
    pub conflict: bool,
}
fn read(c: &Connection, draft: &str) -> Result<(CaseDocument, u64), String> {
    let mut d=c.query_row("SELECT coalesce(d.case_id,d.id),d.base_revision,d.title,d.prompt,coalesce(p.name,d.pending_project_name,''),d.source_url,d.save_seq FROM drafts d LEFT JOIN projects p ON p.id=d.project_id WHERE d.id=?1",[draft],|r|Ok((CaseDocument{id:r.get(0)?,base_revision:r.get::<_,i64>(1)? as u64,title:r.get(2)?,prompt:r.get(3)?,project:r.get(4)?,source_url:r.get(5)?,tags:vec![],result:None,references:vec![]},r.get::<_,i64>(6)? as u64))).optional().map_err(sql)?.ok_or("draft_ended")?;
    let mut tags=c.prepare("SELECT name FROM (SELECT t.name,dt.position FROM draft_tags dt JOIN tags t ON t.id=dt.tag_id WHERE dt.draft_id=?1 UNION ALL SELECT name,position FROM draft_pending_tags WHERE draft_id=?1) ORDER BY position").map_err(sql)?;
    d.0.tags = tags
        .query_map([draft], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    let mut images=c.prepare("SELECT id,asset_id,original_name,role FROM draft_assets WHERE draft_id=?1 ORDER BY position").map_err(sql)?;
    for image in images
        .query_map([draft], |r| {
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
        let (image, role) = image.map_err(sql)?;
        if role == "result" {
            d.0.result = Some(image)
        } else {
            d.0.references.push(image)
        }
    }
    Ok(d)
}
fn contents(
    store: &Store,
    c: &Connection,
    draft: &str,
    d: &CaseDocument,
    seq: u64,
    dirty: bool,
) -> Result<(), String> {
    let project: Option<String> = c
        .query_row(
            "SELECT id FROM projects WHERE name_key=?1",
            [d.project.trim().to_lowercase()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    c.execute("UPDATE drafts SET title=?1,prompt=?2,source_url=?3,project_id=?4,pending_project_name=?5,save_seq=?6,updated_at=?7,dirty=?8 WHERE id=?9",params![d.title,d.prompt,d.source_url,project,if project.is_none(){Some(&d.project)}else{None},i64::try_from(seq).map_err(|_|"draft_seq_overflow")?,now(),dirty,draft]).map_err(sql)?;
    c.execute("DELETE FROM draft_tags WHERE draft_id=?1", [draft])
        .map_err(sql)?;
    c.execute("DELETE FROM draft_pending_tags WHERE draft_id=?1", [draft])
        .map_err(sql)?;
    let mut seen = std::collections::HashSet::new();
    for (position, name) in d.tags.iter().enumerate() {
        let key = name.trim().to_lowercase();
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        let known: Option<String> = c
            .query_row("SELECT id FROM tags WHERE name_key=?1", [key], |r| r.get(0))
            .optional()
            .map_err(sql)?;
        if let Some(tag) = known {
            c.execute(
                "INSERT INTO draft_tags(draft_id,tag_id,position) VALUES(?1,?2,?3)",
                params![draft, tag, position as i64],
            )
            .map_err(sql)?;
        } else {
            c.execute(
                "INSERT INTO draft_pending_tags(draft_id,position,name) VALUES(?1,?2,?3)",
                params![draft, position as i64, name],
            )
            .map_err(sql)?;
        }
    }
    c.execute("DELETE FROM draft_assets WHERE draft_id=?1", [draft])
        .map_err(sql)?;
    for (role, images) in [
        ("result", d.result.iter().collect::<Vec<_>>()),
        ("reference", d.references.iter().collect()),
    ] {
        for (position, image) in images.into_iter().enumerate() {
            uuid::Uuid::parse_str(&image.id).map_err(|_| "invalid_reference_id")?;
            let path: String = c
                .query_row(
                    "SELECT relative_path FROM assets WHERE id=?1",
                    [&image.asset_id],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            store.paths().resolve(&path, "assets")?;
            c.execute("INSERT INTO draft_assets(id,draft_id,asset_id,role,position,original_name) VALUES(?1,?2,?3,?4,?5,?6)",params![image.id,draft,image.asset_id,role,position as i64,image.original_name]).map_err(sql)?;
        }
    }
    Ok(())
}
impl Store {
    pub fn draft_summaries(&self) -> Result<Vec<DraftSummary>, String> {
        let c = self.connection.lock().map_err(|e| e.to_string())?;
        let mut s=c.prepare("SELECT d.id,d.case_id,d.title,substr(d.prompt,1,135),(SELECT count(*) FROM draft_assets a WHERE a.draft_id=d.id AND role='reference'),(SELECT count(*) FROM draft_assets a WHERE a.draft_id=d.id AND role='result'),d.updated_at,CASE WHEN d.case_id IS NULL THEN 'new' WHEN c.id IS NULL THEN 'missing' WHEN c.deleted_at IS NOT NULL THEN 'deleted' ELSE 'live' END,coalesce(c.edit_revision!=d.base_revision,0) FROM drafts d LEFT JOIN cases c ON c.id=d.case_id WHERE d.dirty=1 ORDER BY d.updated_at DESC,d.id").map_err(sql)?;
        let rows = s
            .query_map([], |r| {
                Ok(DraftSummary {
                    id: r.get(0)?,
                    case_id: r.get(1)?,
                    title: r.get(2)?,
                    prompt: r.get(3)?,
                    reference_count: r.get(4)?,
                    result_count: r.get(5)?,
                    updated_at: r.get(6)?,
                    target_status: r.get(7)?,
                    conflict: r.get(8)?,
                })
            })
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        Ok(rows)
    }
    pub fn draft_document(&self, draft: &str) -> Result<(CaseDocument, u64), String> {
        read(&*self.connection.lock().map_err(|e| e.to_string())?, draft)
    }
    fn create_draft(&self, d: &CaseDocument) -> Result<String, String> {
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(sql)?;
        // Never silently replace an unresolved draft for an existing case.
        let previous: Option<(String, bool)> = tx
            .query_row(
                "SELECT id,dirty FROM drafts WHERE case_id=?1",
                [&d.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?;
        if let Some((previous, dirty)) = previous {
            if dirty {
                return Err("case_has_unfinished_draft_recover_first".into());
            }
            tx.execute("DELETE FROM drafts WHERE id=?1", [previous])
                .map_err(sql)?;
        }
        let draft = if d.base_revision == 0 {
            d.id.clone()
        } else {
            id()
        };
        tx.execute(
            "INSERT INTO drafts(id,case_id,base_revision,updated_at,dirty) VALUES(?1,?2,?3,?4,0)",
            params![
                draft,
                if d.base_revision == 0 {
                    None
                } else {
                    Some(&d.id)
                },
                i64::try_from(d.base_revision).map_err(|_| "revision_overflow")?,
                now()
            ],
        )
        .map_err(sql)?;
        contents(self, &tx, &draft, d, 0, false)?;
        tx.commit().map_err(sql)?;
        Ok(draft)
    }
    pub fn update_draft(&self, draft: &str, d: &CaseDocument, seq: u64) -> Result<u64, String> {
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(sql)?;
        let (current, current_seq) = read(&tx, draft)?;
        if current.id != d.id || current.base_revision != d.base_revision {
            return Err("draft_mismatched".into());
        }
        if seq <= current_seq {
            return Err("stale_draft_seq".into());
        }
        contents(self, &tx, draft, d, seq, true)?;
        tx.commit().map_err(sql)?;
        Ok(seq)
    }
    pub fn discard_draft(&self, draft: &str) -> Result<(), String> {
        self.connection
            .lock()
            .map_err(|e| e.to_string())?
            .execute("DELETE FROM drafts WHERE id=?1", [draft])
            .map_err(sql)?;
        Ok(())
    }
    fn recover_document(
        &self,
        draft: &str,
        as_new: bool,
    ) -> Result<(String, CaseDocument, u64), String> {
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(sql)?;
        let (mut d, seq) = read(&tx, draft)?;
        if !as_new && d.base_revision > 0 {
            let live: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM cases WHERE id=?1 AND deleted_at IS NULL)",
                    [&d.id],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if !live {
                return Err("draft_target_missing_or_deleted_restore_case_or_as_new".into());
            }
        }
        let next = if as_new {
            d.id = id();
            d.base_revision = 0;
            tx.execute(
                "INSERT INTO drafts(id,base_revision,updated_at,dirty) VALUES(?1,0,?2,1)",
                params![d.id, now()],
            )
            .map_err(sql)?;
            // Relations use stable reference IDs; remove old rows inside the same transaction first.
            tx.execute("DELETE FROM drafts WHERE id=?1", [draft])
                .map_err(sql)?;
            contents(self, &tx, &d.id, &d, seq, true)?;
            d.id.clone()
        } else {
            draft.to_owned()
        };
        tx.commit().map_err(sql)?;
        Ok((next, d, seq))
    }
}
impl EditSessions {
    pub fn prepare_close(&self) -> Result<(), String> {
        let slot = self.current.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err("editing_not_resolved".into());
        }
        self.invalidate_epoch();
        Ok(())
    }
    pub fn discard_unopened(&self, store: &Store, draft: &str) -> Result<(), String> {
        let slot = self.current.lock().map_err(|e| e.to_string())?;
        if slot.as_ref().is_some_and(|s| s.3.as_deref() == Some(draft)) {
            return Err("draft_is_being_edited".into());
        }
        store.discard_draft(draft)
    }
    pub fn active(&self) -> bool {
        self.current.lock().map(|s| s.is_some()).unwrap_or(true)
    }
    pub fn begin_persistent(
        &self,
        store: &Store,
        case: Option<&str>,
        recover: Option<&str>,
        as_new: bool,
        epoch: u64,
    ) -> Result<EditReceipt, String> {
        let mut slot = self.current.lock().map_err(|e| e.to_string())?;
        if self.epoch() != epoch {
            return Err("edit_window_changed".into());
        }
        if slot.is_some() {
            return Err("another_case_is_being_edited".into());
        }
        let (draft, document, save_seq) = if let Some(draft) = recover {
            store.recover_document(draft, as_new)?
        } else {
            let d = store.edit_document(case)?;
            let draft = store.create_draft(&d)?;
            (draft, d, 0)
        };
        let session_id = id();
        *slot = Some((
            session_id.clone(),
            document.id.clone(),
            document.base_revision,
            Some(draft.clone()),
        ));
        Ok(EditReceipt {
            session_id,
            document,
            draft_id: Some(draft),
            save_seq,
        })
    }
    pub fn stash(
        &self,
        store: &Store,
        token: &str,
        d: &CaseDocument,
        seq: u64,
    ) -> Result<u64, String> {
        let slot = self.current.lock().map_err(|e| e.to_string())?;
        let s = slot
            .as_ref()
            .filter(|s| s.0 == token && s.1 == d.id && s.2 == d.base_revision)
            .ok_or("edit_session_expired_or_mismatched")?;
        store.update_draft(s.3.as_deref().ok_or("no_persistent_draft")?, d, seq)
    }
    pub fn discard(&self, store: &Store, token: &str) -> Result<(), String> {
        let mut slot = self.current.lock().map_err(|e| e.to_string())?;
        let s = slot
            .as_ref()
            .filter(|s| s.0 == token)
            .ok_or("edit_session_expired")?;
        if let Some(draft) = &s.3 {
            store.discard_draft(draft)?;
        }
        *slot = None;
        self.invalidate_epoch();
        Ok(())
    }
}
#[cfg(test)]
mod tests;
