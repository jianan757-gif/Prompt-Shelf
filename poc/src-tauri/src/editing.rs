use crate::Runtime;
use prompt_shelf_poc::{
    cases::{CaseDocument, EditReceipt, SavedCase},
    clipboard_input::{self, ClipboardImage},
    data,
};
use std::{io::Write, path::PathBuf};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedImage {
    #[serde(flatten)]
    pub asset: data::Asset,
    pub original_name: String,
}

fn library_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "library_manager" {
        Ok(())
    } else {
        Err("editing_only_from_library".into())
    }
}
#[tauri::command]
pub async fn begin_case_edit(app:tauri::AppHandle,window:tauri::WebviewWindow,case_id:Option<String>,generation:u64)->Result<EditReceipt,String>{
    library_only(&window)?;let state=app.state::<Runtime>();let edits=state.edits.clone();let host=state.host.clone();
    let epoch={let closing=state.closing.lock().map_err(|e|e.to_string())?;closing.editing_epoch(&edits)?};
    tauri::async_runtime::spawn_blocking(move||host.write(generation,|s|edits.begin_persistent(s,case_id.as_deref(),None,false,epoch))).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub fn cancel_case_edit(app:tauri::AppHandle,window:tauri::WebviewWindow,session_id:String,generation:u64)->Result<(),String>{
    library_only(&window)?;let state=app.state::<Runtime>();state.host.write(generation,|s|state.edits.discard(s,&session_id))
}
#[tauri::command]
pub async fn save_case_draft(app:tauri::AppHandle,window:tauri::WebviewWindow,session_id:String,document:CaseDocument,save_seq:u64,generation:u64)->Result<u64,String>{
    library_only(&window)?;let state=app.state::<Runtime>();let edits=state.edits.clone();let host=state.host.clone();
    tauri::async_runtime::spawn_blocking(move||host.write(generation,|s|edits.stash(s,&session_id,&document,save_seq))).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn list_case_drafts(app:tauri::AppHandle,window:tauri::WebviewWindow,generation:u64)->Result<Vec<prompt_shelf_poc::drafts::DraftSummary>,String>{
    library_only(&window)?;let host=app.state::<Runtime>().host.clone();tauri::async_runtime::spawn_blocking(move||host.read(generation,|s|s.draft_summaries())).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn recover_case_draft(app:tauri::AppHandle,window:tauri::WebviewWindow,draft_id:String,as_new:bool,generation:u64)->Result<EditReceipt,String>{
    library_only(&window)?;let state=app.state::<Runtime>();let edits=state.edits.clone();let host=state.host.clone();let epoch={let closing=state.closing.lock().map_err(|e|e.to_string())?;closing.editing_epoch(&edits)?};
    tauri::async_runtime::spawn_blocking(move||host.write(generation,|s|edits.begin_persistent(s,None,Some(&draft_id),as_new,epoch))).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn discard_case_draft(app:tauri::AppHandle,window:tauri::WebviewWindow,draft_id:String,generation:u64)->Result<(),String>{
    library_only(&window)?;let state=app.state::<Runtime>();let edits=state.edits.clone();let host=state.host.clone();
    tauri::async_runtime::spawn_blocking(move||host.write(generation,|s|edits.discard_unopened(s,&draft_id))).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn commit_case(app:tauri::AppHandle,window:tauri::WebviewWindow,session_id:String,document:CaseDocument,generation:u64)->Result<SavedCase,String>{
    library_only(&window)?;let state=app.state::<Runtime>();let edits=state.edits.clone();let host=state.host.clone();
    let saved=tauri::async_runtime::spawn_blocking(move||host.write(generation,|s|edits.save(s,&session_id,&document))).await.map_err(|e|e.to_string())??;
    state.log("case_saved",serde_json::json!({"caseId":saved.id,"editRevision":saved.edit_revision,"generation":generation}));crate::publish_change(&app,generation,"case-saved",vec![saved.id.clone()]);Ok(saved)
}
#[tauri::command]
pub fn normalize_source_url(window: tauri::WebviewWindow, value: String) -> Result<String, String> {
    library_only(&window)?;
    prompt_shelf_poc::cases::normalize_source(&value)
}

#[tauri::command]
pub async fn import_case_images(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    paths: Option<Vec<String>>,
    clipboard: bool,
    multiple: bool,
    generation: u64,
) -> Result<Vec<ImportedImage>, String> {
    library_only(&window)?;
    let state = app.state::<Runtime>();
    state.edits.check(&session_id)?;
    let host = state.host.clone();
    let grants = state.thumbnails.clone();
    let edits = state.edits.clone();
    let dialog_app = app.clone();
    let images = tauri::async_runtime::spawn_blocking(move || host.write(generation,|store| {
        edits.check(&session_id)?;
        let (paths, png) = if clipboard {
            match clipboard_input::read()? {
                ClipboardImage::Files(paths) => (paths, None),
                ClipboardImage::Png(png) => (vec![], Some(png)),
                ClipboardImage::Dib(dib) => (vec![], Some(clipboard_input::dib_png(&dib)?)),
                ClipboardImage::Empty => return Ok(vec![]),
            }
        } else {
            (
                match paths {
                    Some(paths) => paths.into_iter().map(PathBuf::from).collect(),
                    None => {
                        let picker = dialog_app
                            .dialog()
                            .file()
                            .set_parent(&window)
                            .add_filter("图片", &["png", "jpg", "jpeg", "webp"]);
                        let picked = if multiple {
                            picker.blocking_pick_files().unwrap_or_default()
                        } else {
                            picker.blocking_pick_file().into_iter().collect()
                        };
                        picked
                            .into_iter()
                            .map(|path| path.into_path().map_err(|e| e.to_string()))
                            .collect::<Result<Vec<_>, _>>()?
                    }
                },
                None,
            )
        };
        validate_import_count(multiple, paths.len() + usize::from(png.is_some()))?;
        let mut images = vec![];
        if let Some(png) = png {
            let mut staged = tempfile::NamedTempFile::new_in(store.paths().staging())
                .map_err(|e| e.to_string())?;
            staged.write_all(&png).map_err(|e| e.to_string())?;
            staged.as_file().sync_all().map_err(|e| e.to_string())?;
            images.push(ImportedImage {
                asset: store.import_image(staged.path())?,
                original_name: "剪贴板图片.png".into(),
            });
        }
        for path in paths {
            edits.check(&session_id)?;
            let original_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            images.push(ImportedImage {
                asset: store.import_image(&path)?,
                original_name,
            });
        }
        edits.check(&session_id)?;
        let snapshot=data::Library{cases:vec![],assets:images.iter().map(|image|image.asset.clone()).collect(),projects:vec![],tags:vec![],trash_count:0,ui_preferences:prompt_shelf_poc::catalog::default_preferences(),dictionary:vec![]};
        grants.authorize(&dialog_app,store,&snapshot)?;
        Ok::<_, String>(images)
    }))
    .await
    .map_err(|e|e.to_string())??;
    Ok(images)
}
fn validate_import_count(multiple: bool, count: usize) -> Result<(), String> {
    if !multiple && count > 1 {
        return Err("替换图片时只能导入一张；多图请使用追加参考图".into());
    }
    if count > 500 {
        return Err("too_many_import_images".into());
    }
    Ok(())
}
#[tauri::command]
pub fn open_source_url(window: tauri::WebviewWindow, value: String) -> Result<(), String> {
    library_only(&window)?;
    let url = prompt_shelf_poc::cases::normalize_source(&value)?;
    if url.is_empty() {
        return Err("source_url_empty".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide = |text: &str| {
            std::ffi::OsStr::new(text)
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>()
        };
        let verb = wide("open");
        let url = wide(&url);
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                window.hwnd().map_err(|e| e.to_string())?.0,
                verb.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err(format!("source_open_failed: {}", result as isize));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("source_open_not_implemented_on_this_platform".into())
    }
}
#[cfg(test)]
mod tests {
    use super::validate_import_count;
    #[test]
    fn replacement_is_single_and_append_can_be_multiple() {
        assert!(validate_import_count(false, 0).is_ok());
        assert!(validate_import_count(false, 1).is_ok());
        assert!(validate_import_count(false, 2).is_err());
        assert!(validate_import_count(true, 500).is_ok());
        assert!(validate_import_count(true, 501).is_err());
    }
}
