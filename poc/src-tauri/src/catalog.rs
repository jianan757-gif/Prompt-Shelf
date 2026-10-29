//! Stage C metadata operations. No schema migration or asset/file deletion.
use crate::store::Store;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashSet;

fn sql(error: rusqlite::Error) -> String {
    format!("sqlite: {error}")
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn table(kind: &str) -> Result<&'static str, String> {
    match kind {
        "projects" => Ok("projects"),
        "tags" => Ok("tags"),
        _ => Err("invalid_catalog_kind".into()),
    }
}
fn name(raw: &str) -> Result<(String, String), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("catalog_name_required".into());
    }
    if trimmed.chars().count() > 120 || trimmed.chars().any(char::is_control) {
        return Err("invalid_catalog_name".into());
    }
    Ok((trimmed.into(), trimmed.to_lowercase()))
}
fn known(c: &Connection, table: &str, id: &str) -> Result<(String, String), String> {
    c.query_row(
        &format!("SELECT name,name_key FROM {table} WHERE id=?1"),
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(sql)?
    .ok_or("catalog_changed_refresh_and_retry".into())
}
fn check_duplicate(
    c: &Connection,
    table: &str,
    key: &str,
    except: Option<&str>,
) -> Result<(), String> {
    let duplicate: bool = c
        .query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {table} WHERE name_key=?1 AND (?2 IS NULL OR id!=?2))"
            ),
            params![key, except],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if duplicate {
        Err("catalog_name_exists".into())
    } else {
        Ok(())
    }
}
fn quick_ids(c: &Connection) -> Result<Vec<String>, String> {
    let mut statement = c
        .prepare("SELECT id FROM tags WHERE is_quick=1 ORDER BY quick_order,created_at,id")
        .map_err(sql)?;
    let rows = statement
        .query_map([], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    Ok(rows)
}
fn write_quick_order(c: &Connection, ids: &[String]) -> Result<(), String> {
    for (position, id) in ids.iter().enumerate() {
        c.execute(
            "UPDATE tags SET quick_order=?1 WHERE id=?2",
            params![position as i64, id],
        )
        .map_err(sql)?;
    }
    Ok(())
}
// A name could have been unresolved when stashed and registered later. Bind it before a
// rename, or remove it on delete, so recovery cannot resurrect an explicitly deleted name.
fn pending_projects(c: &Connection, key: &str, bind: Option<&str>) -> Result<(), String> {
    let rows = {
        let mut q = c
            .prepare(
                "SELECT id,pending_project_name FROM drafts WHERE pending_project_name IS NOT NULL",
            )
            .map_err(sql)?;
        let rows = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql)?;
        rows
    };
    for (draft, value) in rows {
        if value.trim().to_lowercase() == key {
            c.execute(
                "UPDATE drafts SET project_id=?1,pending_project_name=NULL WHERE id=?2",
                params![bind, draft],
            )
            .map_err(sql)?;
        }
    }
    Ok(())
}
fn pending_tags(c: &Connection, key: &str, bind: Option<&str>) -> Result<(), String> {
    let rows = {
        let mut q = c
            .prepare("SELECT draft_id,position,name FROM draft_pending_tags")
            .map_err(sql)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql)?;
        rows
    };
    for (draft, position, value) in rows {
        if value.trim().to_lowercase() != key {
            continue;
        }
        if let Some(tag) = bind {
            let existing_position: Option<i64> = c
                .query_row(
                    "SELECT position FROM draft_tags WHERE draft_id=?1 AND tag_id=?2",
                    params![draft, tag],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?;
            if let Some(existing) = existing_position {
                if position < existing {
                    c.execute(
                        "UPDATE draft_tags SET position=?1 WHERE draft_id=?2 AND tag_id=?3",
                        params![position, draft, tag],
                    )
                    .map_err(sql)?;
                }
            } else {
                c.execute(
                    "INSERT INTO draft_tags(draft_id,tag_id,position) VALUES(?1,?2,?3)",
                    params![draft, tag, position],
                )
                .map_err(sql)?;
            }
        }
        c.execute(
            "DELETE FROM draft_pending_tags WHERE draft_id=?1 AND position=?2",
            params![draft, position],
        )
        .map_err(sql)?;
    }
    Ok(())
}

pub fn default_preferences() -> Value {
    json!({"libraryLayout":"regular","librarySort":"recent","catalogColumns":"single","tagSort":"time","projectSort":"time","railWidth":224,"secondaryWidth":320,"expandedCatalog":null,"quickIntervalMs":700})
}
pub fn default_dictionary() -> Vec<Vec<String>> {
    [
        vec!["换色", "色调迁移", "配色替换"],
        vec!["保构图", "构图锁定", "结构锁定"],
        vec!["精修", "高清精修", "商业精修"],
        vec!["扩图", "画面扩展", "图像扩展", "外扩"],
        vec!["去背", "去背景", "背景移除"],
        vec!["产品渲染", "商品渲染", "商业CG"],
    ]
    .into_iter()
    .map(|group| group.into_iter().map(str::to_owned).collect())
    .collect()
}
fn valid_patch(patch: &Value) -> Result<(), String> {
    let fields = patch
        .as_object()
        .ok_or("preferences_patch_requires_object")?;
    for (key, value) in fields {
        let valid = match key.as_str() {
            "libraryLayout" => matches!(value.as_str(), Some("regular" | "masonry")),
            "librarySort" => matches!(value.as_str(), Some("recent" | "name" | "new")),
            "catalogColumns" => matches!(value.as_str(), Some("single" | "double")),
            "tagSort" | "projectSort" => matches!(value.as_str(), Some("time" | "name")),
            "expandedCatalog" => {
                value.is_null() || matches!(value.as_str(), Some("tags" | "projects"))
            }
            "railWidth" => value.as_i64().is_some_and(|v| (172..=340).contains(&v)),
            "secondaryWidth" => value.as_i64().is_some_and(|v| (278..=680).contains(&v)),
            "quickIntervalMs" => value.as_i64().is_some_and(|v| (50..=10000).contains(&v)),
            _ => false,
        };
        if !valid {
            return Err(format!("invalid_preference: {key}"));
        }
    }
    Ok(())
}
pub(crate) fn preferences_from(c: &Connection) -> Result<Value, String> {
    let mut preferences = default_preferences();
    let stored: Option<String> = c
        .query_row(
            "SELECT value_json FROM settings WHERE key='ui_preferences'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if let Some(stored) = stored {
        let patch: Value =
            serde_json::from_str(&stored).map_err(|_| "invalid_stored_preferences")?;
        valid_patch(&patch)?;
        for (key, value) in patch.as_object().unwrap() {
            preferences[key] = value.clone();
        }
    }
    Ok(preferences)
}
pub(crate) fn dictionary_from(c: &Connection) -> Result<Vec<Vec<String>>, String> {
    let initialized: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key='dictionary_initialized')",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let mut q = c
        .prepare("SELECT terms_json FROM synonym_groups ORDER BY sort_order,id")
        .map_err(sql)?;
    let raw = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    if raw.is_empty() && !initialized {
        return Ok(default_dictionary());
    }
    let groups = raw
        .into_iter()
        .map(|value| {
            serde_json::from_str::<Vec<String>>(&value)
                .map_err(|_| "invalid_stored_dictionary".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalize_dictionary(groups)
}
fn normalize_dictionary(groups: Vec<Vec<String>>) -> Result<Vec<Vec<String>>, String> {
    if groups.len() > 100 {
        return Err("dictionary_too_many_groups".into());
    }
    let mut output = vec![];
    let mut global = HashSet::new();
    for group in groups {
        if group.len() > 30 {
            return Err("dictionary_too_many_terms".into());
        }
        let mut local = HashSet::new();
        let mut terms = vec![];
        for term in group {
            let value = term.trim();
            if value.is_empty() {
                continue;
            }
            if value.chars().count() > 120 || value.chars().any(char::is_control) {
                return Err("invalid_dictionary_term".into());
            }
            let key = value.to_lowercase();
            if !local.insert(key.clone()) {
                continue;
            }
            if !global.insert(key) {
                return Err("dictionary_term_in_multiple_groups".into());
            }
            terms.push(value.to_owned());
        }
        if terms.len() < 2 {
            return Err("dictionary_group_requires_two_terms".into());
        }
        output.push(terms);
    }
    Ok(output)
}
impl Store {
    pub fn catalog_create(&self, kind: &str, raw: &str) -> Result<String, String> {
        let table = table(kind)?;
        let (name, key) = name(raw)?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        check_duplicate(&tx, table, &key, None)?;
        let id = id();
        tx.execute(&format!("INSERT INTO {table}(id,name,name_key,sort_order,created_at) VALUES(?1,?2,?3,(SELECT coalesce(max(sort_order),-1)+1 FROM {table}),?4)"),params![id,name,key,now()]).map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(id)
    }
    pub fn catalog_rename(&self, kind: &str, id: &str, raw: &str) -> Result<(), String> {
        let table = table(kind)?;
        let (name, key) = name(raw)?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        let (_, old) = known(&tx, table, id)?;
        check_duplicate(&tx, table, &key, Some(id))?;
        if kind == "projects" {
            pending_projects(&tx, &old, Some(id))?;
            pending_projects(&tx, &key, Some(id))?
        } else {
            pending_tags(&tx, &old, Some(id))?;
            pending_tags(&tx, &key, Some(id))?
        }
        tx.execute(
            &format!("UPDATE {table} SET name=?1,name_key=?2 WHERE id=?3"),
            params![name, key, id],
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)
    }
    pub fn catalog_delete(&self, kind: &str, ids: &[String]) -> Result<(), String> {
        let table = table(kind)?;
        if ids.is_empty() || ids.len() > 1000 {
            return Err("catalog_selection_required_or_too_large".into());
        }
        let mut seen = HashSet::new();
        if ids.iter().any(|id| !seen.insert(id)) {
            return Err("duplicate_catalog_id".into());
        }
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        let keys = ids
            .iter()
            .map(|id| known(&tx, table, id).map(|(_, key)| key))
            .collect::<Result<Vec<_>, _>>()?;
        for (id, key) in ids.iter().zip(keys) {
            if kind == "projects" {
                pending_projects(&tx, &key, None)?
            } else {
                pending_tags(&tx, &key, None)?
            }
            tx.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id])
                .map_err(sql)?;
        }
        if kind == "tags" {
            write_quick_order(&tx, &quick_ids(&tx)?)?
        }
        tx.commit().map_err(sql)
    }
    pub fn set_quick_tag(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        known(&tx, "tags", id)?;
        let current: bool = tx
            .query_row("SELECT is_quick FROM tags WHERE id=?1", [id], |r| r.get(0))
            .map_err(sql)?;
        if current == enabled {
            return tx.commit().map_err(sql);
        }
        let mut ids = quick_ids(&tx)?;
        ids.retain(|current| current != id);
        if enabled {
            ids.push(id.into())
        }
        tx.execute(
            "UPDATE tags SET is_quick=?1 WHERE id=?2",
            params![enabled, id],
        )
        .map_err(sql)?;
        write_quick_order(&tx, &ids)?;
        tx.commit().map_err(sql)
    }
    pub fn reorder_quick_tags(&self, ids: &[String]) -> Result<(), String> {
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        let current = quick_ids(&tx)?;
        let wanted = ids.iter().collect::<HashSet<_>>();
        if wanted.len() != ids.len()
            || current.len() != ids.len()
            || current.iter().any(|id| !wanted.contains(id))
        {
            return Err("quick_tags_changed_refresh_and_retry".into());
        }
        write_quick_order(&tx, ids)?;
        tx.commit().map_err(sql)
    }
    pub fn set_favorite(&self, case_id: &str, favorite: bool) -> Result<(), String> {
        let c = self.connection.lock().map_err(|e| e.to_string())?;
        if c.execute(
            "UPDATE cases SET favorite=?1 WHERE id=?2 AND deleted_at IS NULL",
            params![favorite, case_id],
        )
        .map_err(sql)?
            != 1
        {
            return Err("case_missing_or_deleted".into());
        }
        Ok(())
    }
    pub fn ui_preferences(&self) -> Result<Value, String> {
        preferences_from(&*self.connection.lock().map_err(|e| e.to_string())?)
    }
    pub fn set_ui_preferences(&self, patch: Value) -> Result<(), String> {
        valid_patch(&patch)?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        let mut preferences = preferences_from(&tx)?;
        for (key, value) in patch.as_object().unwrap() {
            preferences[key] = value.clone();
        }
        tx.execute("INSERT INTO settings(key,value_json) VALUES('ui_preferences',?1) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",[preferences.to_string()]).map_err(sql)?;
        tx.commit().map_err(sql)
    }
    pub fn dictionary(&self) -> Result<Vec<Vec<String>>, String> {
        dictionary_from(&*self.connection.lock().map_err(|e| e.to_string())?)
    }
    pub fn save_dictionary(&self, groups: Vec<Vec<String>>) -> Result<(), String> {
        let groups = normalize_dictionary(groups)?;
        let mut c = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        tx.execute("DELETE FROM synonym_groups", []).map_err(sql)?;
        for (position, group) in groups.into_iter().enumerate() {
            tx.execute(
                "INSERT INTO synonym_groups(id,terms_json,sort_order) VALUES(?1,?2,?3)",
                params![
                    id(),
                    serde_json::to_string(&group).map_err(|e| e.to_string())?,
                    position as i64
                ],
            )
            .map_err(sql)?;
        }
        tx.execute("INSERT INTO settings(key,value_json) VALUES('dictionary_initialized','true') ON CONFLICT(key) DO UPDATE SET value_json='true'",[]).map_err(sql)?;
        tx.commit().map_err(sql)
    }
}
#[cfg(test)]
mod tests;
