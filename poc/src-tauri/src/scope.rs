use crate::{data::Library, store::Store};
use std::{collections::HashSet, path::PathBuf, sync::Mutex};
use tauri::Manager;

/// Track only the exact thumbnail files granted by this running app.
#[derive(Default)]
pub struct ThumbnailGrants(Mutex<HashSet<PathBuf>>);
impl ThumbnailGrants {
    pub fn authorize<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        store: &Store,
        library: &Library,
    ) -> Result<(), String> {
        let files = store.thumbnail_files(library)?;
        let mut granted = self.0.lock().map_err(|e| e.to_string())?;
        for path in files {
            app.asset_protocol_scope()
                .allow_file(&path)
                .map_err(|e| e.to_string())?;
            granted.insert(path);
        }
        Ok(())
    }
    pub fn revoke_previous<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        current_storage: &std::path::Path,
    ) -> Result<(), String> {
        let mut granted = self.0.lock().map_err(|e| e.to_string())?;
        let previous = granted
            .iter()
            .filter(|path| !path.starts_with(current_storage))
            .cloned()
            .collect::<Vec<_>>();
        for path in previous {
            app.asset_protocol_scope()
                .forbid_file(&path)
                .map_err(|e| e.to_string())?;
            granted.remove(&path);
        }
        Ok(())
    }
}

/// Startup and refresh share this exact-file policy. No directory grants.
pub fn authorize_thumbnails<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &Store,
    library: &Library,
) -> Result<(), String> {
    for path in store.thumbnail_files(library)? {
        app.asset_protocol_scope()
            .allow_file(path)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
