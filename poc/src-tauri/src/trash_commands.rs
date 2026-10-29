use crate::{publish_change, Runtime};
use prompt_shelf_poc::{
    catalog, data, store::Store, trash::TrashCase,
};
use tauri::Manager;

fn validate<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
 ) -> Result<(), String> {
    if window.label() != "library_manager" {
        return Err("trash_only_from_library".into());
    }
    Ok(())
}
async fn change(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    case_id: String,
    action: fn(&Store, &str) -> Result<(), String>,
    reason: &'static str,
) -> Result<(), String> {
    validate(&window)?;
    let state = app.state::<Runtime>();
    {
        let closing = state.closing.lock().map_err(|e| e.to_string())?;
        if closing.pending.is_some() {
            return Err("library_close_pending".into());
        }
    }
    let edits = state.edits.clone();
    let store = state.host.clone();
    let entity = case_id.clone();
    tauri::async_runtime::spawn_blocking(move || store.write(generation,|s|edits.while_idle(|| action(s, &case_id))))
        .await
        .map_err(|e| e.to_string())??;
    publish_change(&app, generation, reason, vec![entity]);
    Ok(())
}
#[tauri::command]
pub async fn soft_delete_case(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    case_id: String,
) -> Result<(), String> {
    change(
        app,
        window,
        generation,
        case_id,
        Store::soft_delete_case,
        "case-deleted",
    )
    .await
}
#[tauri::command]
pub async fn restore_case(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    case_id: String,
) -> Result<(), String> {
    change(
        app,
        window,
        generation,
        case_id,
        Store::restore_case,
        "case-restored",
    )
    .await
}
#[tauri::command]
pub async fn permanently_delete_case(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    case_id: String,
) -> Result<(), String> {
    change(
        app,
        window,
        generation,
        case_id,
        Store::permanently_delete_case,
        "case-permanently-deleted",
    )
    .await
}
#[tauri::command]
pub async fn get_case_delete_draft(app:tauri::AppHandle,window:tauri::WebviewWindow,case_id:String,generation:u64)->Result<Option<String>,String>{validate(&window)?;let host=app.state::<Runtime>().host.clone();tauri::async_runtime::spawn_blocking(move||host.read(generation,|s|s.case_delete_draft(&case_id))).await.map_err(|e|e.to_string())?}
#[tauri::command]
pub async fn get_trash_index(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64)->Result<Vec<TrashCase>,String>{validate(&window)?;let host=app.state::<Runtime>().host.clone();tauri::async_runtime::spawn_blocking(move||host.read(generation,|s|s.trash_index())).await.map_err(|e|e.to_string())?}
#[tauri::command]
pub async fn get_trash_assets(app:tauri::AppHandle,window:tauri::WebviewWindow,case_ids:Vec<String>,generation:u64)->Result<Vec<data::Asset>,String>{
    validate(&window)?;let state=app.state::<Runtime>();let host=state.host.clone();let grants=state.thumbnails.clone();
    tauri::async_runtime::spawn_blocking(move||host.read(generation,|store|{let snapshot=data::Library{cases:vec![],assets:store.trash_assets(&case_ids)?,projects:vec![],tags:vec![],trash_count:0,ui_preferences:catalog::default_preferences(),dictionary:vec![]};grants.authorize(&app,store,&snapshot)?;Ok(snapshot.assets)})).await.map_err(|e|e.to_string())?
}
#[cfg(test)]mod tests{#[test]fn trash_requires_main_library(){use tauri::{test::mock_app,WebviewUrl,WebviewWindowBuilder};let app=mock_app();let main=WebviewWindowBuilder::new(&app,"library_manager",WebviewUrl::App("index.html".into())).build().unwrap();let quick=WebviewWindowBuilder::new(&app,"quick_panel",WebviewUrl::App("index.html".into())).build().unwrap();assert!(super::validate(&main).is_ok());assert!(super::validate(&quick).is_err());}}