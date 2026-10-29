//! Recycle-bin mutations change metadata only. Physical cleanup has one startup entry point.
use crate::{data, store::Store};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

fn sql(error: rusqlite::Error) -> String {
    format!("sqlite: {error}")
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashCase {
    #[serde(flatten)]
    pub case: data::Case,
    pub deleted_at: i64,
    pub deleted_project_name: String,
    pub shared_image_count: u64,
    pub unfinished_draft_id: Option<String>,
}

impl Store {
    /// Includes clean drafts, which are omitted from the recovery summary but still hold work.
    pub fn case_delete_draft(&self, id: &str) -> Result<Option<String>, String> {
        let c = self.connection.lock().map_err(|e| e.to_string())?;
        let exists: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM cases WHERE id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if !exists {
            return Err("case_missing_or_deleted".into());
        }
        c.query_row("SELECT id FROM drafts WHERE case_id=?1", [id], |r| r.get(0))
            .optional()
            .map_err(sql)
    }

    /// The count is distinct images used by another case entity, including other trash cases.
    /// Draft-only references protect files during GC but are deliberately not this UI count.
    pub fn trash_index(&self) -> Result<Vec<TrashCase>, String> {
        let c = self.connection.lock().map_err(|e| e.to_string())?;
        let mut statement = c.prepare("SELECT c.id,c.title,c.prompt,coalesce(p.name,''),c.favorite,c.use_count,c.source_url,c.created_at,c.updated_at,c.last_used_at,c.edit_revision,c.project_id,c.deleted_at,coalesce(c.deleted_project_name,''),(SELECT count(DISTINCT own.asset_id) FROM case_assets own WHERE own.case_id=c.id AND EXISTS(SELECT 1 FROM case_assets other WHERE other.asset_id=own.asset_id AND other.case_id!=c.id)),(SELECT id FROM drafts WHERE case_id=c.id) FROM cases c LEFT JOIN projects p ON p.id=c.project_id WHERE c.deleted_at IS NOT NULL ORDER BY c.deleted_at DESC,c.id").map_err(sql)?;
        let mut rows = statement
            .query_map([], |r| {
                Ok(TrashCase {
                    case: data::Case {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        prompt: r.get(2)?,
                        project: r.get(3)?,
                        favorite: r.get(4)?,
                        used: r.get::<_, i64>(5)? as u64,
                        source_url: r.get(6)?,
                        created_at: r.get(7)?,
                        updated_at: r.get(8)?,
                        last_used_at: r.get(9)?,
                        edit_revision: r.get::<_, i64>(10)? as u64,
                        project_id: r.get(11)?,
                        tag_ids: vec![],
                        tags: vec![],
                        result: None,
                        refs: vec![],
                    },
                    deleted_at: r.get(12)?,
                    deleted_project_name: r.get(13)?,
                    shared_image_count: r.get::<_, i64>(14)? as u64,
                    unfinished_draft_id: r.get(15)?,
                })
            })
            .map_err(sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql)?;
        drop(statement);
        for row in &mut rows {
            let mut tags = c.prepare("SELECT t.id,t.name FROM case_tags ct JOIN tags t ON t.id=ct.tag_id WHERE ct.case_id=?1 ORDER BY ct.position").map_err(sql)?;
            for pair in tags
                .query_map([&row.case.id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(sql)?
            {
                let (id, name) = pair.map_err(sql)?;
                row.case.tag_ids.push(id);
                row.case.tags.push(name);
            }
            let mut images = c
                .prepare(
                    "SELECT asset_id,role FROM case_assets WHERE case_id=?1 ORDER BY role,position",
                )
                .map_err(sql)?;
            for pair in images
                .query_map([&row.case.id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(sql)?
            {
                let (asset, role) = pair.map_err(sql)?;
                if role == "result" {
                    row.case.result = Some(asset);
                } else {
                    row.case.refs.push(asset);
                }
            }
        }
        Ok(rows)
    }

    pub fn trash_assets(&self, ids: &[String]) -> Result<Vec<data::Asset>, String> {
        if ids.len() > 120 {
            return Err("case_asset_batch_too_large".into());
        }
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        let c = self.connection.lock().map_err(|e| e.to_string())?;
        let placeholders = (1..=ids.len())
            .map(|n| format!("?{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!("SELECT DISTINCT a.id,a.sha256,a.relative_path,a.width,a.height FROM assets a JOIN case_assets ca ON ca.asset_id=a.id JOIN cases c ON c.id=ca.case_id WHERE c.deleted_at IS NOT NULL AND c.id IN ({placeholders}) ORDER BY a.id");
        let mut statement = c.prepare(&query).map_err(sql)?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(ids), |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql)?;
        drop(statement);
        drop(c);
        self.asset_projection(rows, true)
    }

    pub fn soft_delete_case(&self, id: &str) -> Result<(), String> {
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(sql)?;
        require_case(&tx, id, false)?;
        require_no_draft(&tx, id)?;
        tx.execute("UPDATE cases SET deleted_at=?1,deleted_project_name=(SELECT name FROM projects WHERE id=cases.project_id) WHERE id=?2", params![now(), id]).map_err(sql)?;
        tx.commit().map_err(sql)
    }

    /// Undo uses this same persisted operation. A missing project remains independent via FK.
    pub fn restore_case(&self, id: &str) -> Result<(), String> {
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(sql)?;
        require_case(&tx, id, true)?;
        tx.execute(
            "UPDATE cases SET deleted_at=NULL,deleted_project_name=NULL WHERE id=?1",
            [id],
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)
    }

    /// The caller must explicitly resolve the related draft before invoking this operation.
    pub fn permanently_delete_case(&self, id: &str) -> Result<(), String> {
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(sql)?;
        require_case(&tx, id, true)?;
        require_no_draft(&tx, id)?;
        tx.execute("DELETE FROM cases WHERE id=?1", [id])
            .map_err(sql)?;
        crate::gc::mark_unreferenced(&tx)?;
        tx.commit().map_err(sql)
    }
}

fn require_case(c: &rusqlite::Connection, id: &str, deleted: bool) -> Result<(), String> {
    let state = c
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM cases WHERE id=?1",
            [id],
            |r| r.get::<_, bool>(0),
        )
        .optional()
        .map_err(sql)?;
    match state {
        Some(current) if current == deleted => Ok(()),
        Some(_) if deleted => Err("case_not_in_trash".into()),
        _ => Err("case_missing_or_deleted".into()),
    }
}
fn require_no_draft(c: &rusqlite::Connection, id: &str) -> Result<(), String> {
    let draft: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM drafts WHERE case_id=?1)",
            [id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if draft {
        Err("case_has_unfinished_draft_resolve_first".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
