//! One resource lifecycle coordinator. No directory scans and no persistent ref counts.
//! Fixed lock order: edit session -> resources -> DB. Physical cleanup is a startup action.
use crate::{paths::DataPaths, store::Store};
use rusqlite::Connection;
use serde::Serialize;
use std::{collections::HashSet, path::PathBuf};

#[derive(Default)]
pub(crate) struct ResourceLifecycle {
    pub(crate) imported: HashSet<String>,
    pub(crate) output: HashSet<String>,
}

/// The caller must hold a reliable clipboard snapshot for the whole cleanup operation.
/// A busy/unsupported clipboard, unreadable format or malformed CF_HDROP is Unknown.
pub enum ClipboardProtection {
    Unknown,
    Files(Vec<PathBuf>),
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcReport {
    pub candidates: usize,
    pub removed: usize,
    pub retained: usize,
    pub failures: Vec<GcFailure>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcFailure {
    pub asset_id: String,
    pub error: String,
}
fn sql(e: rusqlite::Error) -> String { format!("sqlite: {e}") }

/// Also called inside relation-changing transactions. A pending row can become live again.
pub(crate) fn mark_unreferenced(c: &Connection) -> Result<(), String> {
    c.execute("UPDATE assets SET gc_state=CASE WHEN EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=assets.id) OR EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=assets.id) THEN 'live' ELSE 'pending' END WHERE gc_state != CASE WHEN EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=assets.id) OR EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=assets.id) THEN 'live' ELSE 'pending' END", []).map_err(sql)?;
    Ok(())
}

impl Store {
    /// GUI calls this before accepting tasks, with the clipboard held stable by its platform guard.
    /// Tests inject their own protection; this function never reads the system clipboard.
    pub fn collect_garbage(&self, clipboard: ClipboardProtection) -> Result<GcReport, String> {
        let resources = self.resources.lock().map_err(|e| e.to_string())?;
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        mark_unreferenced(&connection)?;
        let rows = {
            let mut s = connection.prepare("SELECT id,sha256,extension,relative_path FROM assets WHERE gc_state='pending' AND NOT EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=assets.id) AND NOT EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=assets.id) ORDER BY id").map_err(sql)?;
            let rows = s.query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))
                .map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
            rows
        };
        // If even one clipboard path cannot be inspected reliably, preserve all candidates.
        let clipboard_paths = match clipboard {
            ClipboardProtection::Unknown => None,
            ClipboardProtection::Files(paths) => paths.into_iter().map(|p| {
                if !p.is_absolute() { return Err("clipboard_path_not_absolute"); }
                dunce::canonicalize(p).map_err(|_| "clipboard_path_unavailable")
            }).collect::<Result<Vec<_>,_>>().ok(),
        };
        let mut report = GcReport { candidates: rows.len(), ..Default::default() };
        for (asset_id, hash, extension, relative) in rows {
            if resources.imported.contains(&asset_id) || resources.output.contains(&asset_id) || clipboard_paths.is_none() {
                report.retained += 1;
                continue;
            }
            let result = (|| {
                // Do not trust a manually altered database path, even though schema validates it.
                if relative != DataPaths::asset_relative(&hash, &extension)? { return Err("invalid_gc_path".into()); }
                let expected = self.paths().destination(&relative);
                // Lexical path is useful when a previous attempt already removed the file.
                // Existing candidates additionally use canonical identity for clipboard aliases.
                let identity = dunce::canonicalize(&expected).unwrap_or(expected);
                if clipboard_paths.as_ref().is_some_and(|paths| paths.contains(&identity)) { return Ok(false); }
                let thumbnail = DataPaths::thumb_relative(&hash)?;
                // Remove cache first. On any failure the row remains pending and retryable.
                self.paths().remove_managed_file(&thumbnail, "cache/thumbs")?;
                self.paths().remove_managed_file(&relative, "assets")?;
                let count=connection.execute("DELETE FROM assets WHERE id=?1 AND gc_state='pending' AND NOT EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=assets.id) AND NOT EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=assets.id)", [&asset_id]).map_err(sql)?;
                if count != 1 { return Err("gc_reference_changed".into()); }
                Ok(true)
            })();
            match result {
                Ok(true) => report.removed += 1,
                Ok(false) => report.retained += 1,
                Err(error) => report.failures.push(GcFailure { asset_id, error }),
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
