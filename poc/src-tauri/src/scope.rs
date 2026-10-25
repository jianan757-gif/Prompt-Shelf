use crate::{data::Library, store::Store};
use tauri::Manager;

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
