use crate::{Runtime,publish_change};
use prompt_shelf_poc::{catalog,data,scope::authorize_thumbnails};
use tauri::Manager;
fn validate<R:tauri::Runtime>(window:&tauri::WebviewWindow<R>,generation:u64,library_only:bool)->Result<(),String>{
    if generation!=1{return Err("stale_library_generation".into())}
    if !["library_manager","quick_panel"].contains(&window.label())||library_only&&window.label()!="library_manager"{return Err("catalog_command_window_not_allowed".into())}Ok(())
}
async fn catalog_change<T:Send+'static>(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,change:impl FnOnce(&prompt_shelf_poc::store::Store)->Result<T,String>+Send+'static)->Result<T,String>{
    validate(&window,generation,true)?;let state=app.state::<Runtime>();
    {let closing=state.closing.lock().map_err(|e|e.to_string())?;if closing.pending.is_some(){return Err("library_close_pending".into())}}
    let edits=state.edits.clone();let store=state.store.clone();
    let result=tauri::async_runtime::spawn_blocking(move||edits.while_idle(||change(&store))).await.map_err(|e|e.to_string())??;
    publish_change(&app,"catalog-changed",vec![]);Ok(result)
}
#[tauri::command]pub async fn catalog_create(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,kind:String,name:String)->Result<String,String>{catalog_change(app,window,generation,move|s|s.catalog_create(&kind,&name)).await}
#[tauri::command]pub async fn catalog_rename(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,kind:String,id:String,name:String)->Result<(),String>{catalog_change(app,window,generation,move|s|s.catalog_rename(&kind,&id,&name)).await}
#[tauri::command]pub async fn catalog_delete(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,kind:String,ids:Vec<String>)->Result<(),String>{catalog_change(app,window,generation,move|s|s.catalog_delete(&kind,&ids)).await}
#[tauri::command]pub async fn set_quick_tag(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,id:String,enabled:bool)->Result<(),String>{catalog_change(app,window,generation,move|s|s.set_quick_tag(&id,enabled)).await}
#[tauri::command]pub async fn reorder_quick_tags(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,ids:Vec<String>)->Result<(),String>{catalog_change(app,window,generation,move|s|s.reorder_quick_tags(&ids)).await}
#[tauri::command]pub async fn set_favorite(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,case_id:String,favorite:bool)->Result<(),String>{
    validate(&window,generation,false)?;let store=app.state::<Runtime>().store.clone();let entity=case_id.clone();tauri::async_runtime::spawn_blocking(move||store.set_favorite(&case_id,favorite)).await.map_err(|e|e.to_string())??;publish_change(&app,"favorite-changed",vec![entity]);Ok(())
}
#[tauri::command]pub async fn set_ui_preferences(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,patch:serde_json::Value)->Result<(),String>{
    validate(&window,generation,false)?;let store=app.state::<Runtime>().store.clone();tauri::async_runtime::spawn_blocking(move||store.set_ui_preferences(patch)).await.map_err(|e|e.to_string())??;publish_change(&app,"preferences-changed",vec![]);Ok(())
}
#[tauri::command]pub async fn save_dictionary(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64,groups:Vec<Vec<String>>)->Result<(),String>{
    validate(&window,generation,true)?;let store=app.state::<Runtime>().store.clone();tauri::async_runtime::spawn_blocking(move||store.save_dictionary(groups)).await.map_err(|e|e.to_string())??;publish_change(&app,"dictionary-changed",vec![]);Ok(())
}
#[tauri::command]pub async fn get_library_index(state:tauri::State<'_,Runtime>)->Result<data::Library,String>{let store=state.store.clone();tauri::async_runtime::spawn_blocking(move||store.library_index()).await.map_err(|e|e.to_string())?}
#[tauri::command]pub async fn get_case_assets(app:tauri::AppHandle,window:tauri::WebviewWindow,case_ids:Vec<String>)->Result<Vec<data::Asset>,String>{
    validate(&window,1,false)?;let store=app.state::<Runtime>().store.clone();let reader=store.clone();let assets=tauri::async_runtime::spawn_blocking(move||reader.assets_for_cases(&case_ids)).await.map_err(|e|e.to_string())??;
    let snapshot=data::Library{cases:vec![],assets,projects:vec![],tags:vec![],trash_count:0,ui_preferences:catalog::default_preferences(),dictionary:vec![]};authorize_thumbnails(&app,&store,&snapshot)?;Ok(snapshot.assets)
}
#[cfg(test)]mod tests{
    #[test]fn generation_and_window_are_required(){use tauri::{test::mock_app,WebviewUrl,WebviewWindowBuilder};let app=mock_app();let quick=WebviewWindowBuilder::new(&app,"quick_panel",WebviewUrl::App("index.html".into())).build().unwrap();assert!(super::validate(&quick,1,false).is_ok());assert!(super::validate(&quick,1,true).is_err());assert!(super::validate(&quick,2,false).is_err());}
}
