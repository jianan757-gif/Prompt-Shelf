#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod data;
mod escape;
mod platform;
mod queue;
use platform::{NativePlatform, PasteItem, PastePlatform};
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

type Target = <NativePlatform as PastePlatform>::Target;
const BUILD_REVISION: &str = "4.3-round7";
struct Runtime {
    platform: Arc<NativePlatform>,
    queue: Arc<queue::Queue>,
    escape: Mutex<escape::EscapeSession>,
    target: Mutex<Option<Target>>,
    library: data::Library,
    log: Mutex<std::fs::File>,
    log_path: PathBuf,
    token: AtomicU64,
    wake: Mutex<Option<(u64, Instant, String, bool)>>,
    library_start: Mutex<Option<Instant>>,
    library_creating: Mutex<bool>,
    tray_status: Mutex<Option<MenuItem<tauri::Wry>>>,
}
impl Runtime {
    fn log(&self, kind: &str, value: serde_json::Value) {
        let line = serde_json::json!({"timestampMs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),"pid":std::process::id(),"kind":kind,"data":value});
        if let Ok(mut file) = self.log.lock() {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }
}
fn can_preserve_previous(from_hotkey: bool, error: &str) -> bool {
    from_hotkey && error == "quick_panel_keep_previous_target"
}
fn escape_status(app: &tauri::AppHandle) {
    let state = app.state::<Runtime>();
    let session = state.escape.lock().unwrap();
    state.log("escape_session",serde_json::json!({"visible":session.visible,"registered":session.registered,"error":session.error}));
    let _ = app.emit("escape-status", serde_json::json!({"registered":session.registered,"error":session.error}));
    if let Some(error) = &session.error {
        state.log("escape_error", serde_json::json!({"error":error,"visible":session.visible,"registered":session.registered}));
        if let Some(status) = state.tray_status.lock().unwrap().as_ref() { let _ = status.set_text(error); }
    }
}
fn close_panel(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Runtime>();
    state.queue.cancel();
    let window = app.get_webview_window("quick_panel").ok_or("panel_missing")?;
    let mut session = state.escape.lock().unwrap();
    if session.registered {
        if let Err(error) = app.global_shortcut().unregister(Shortcut::new(None,Code::Escape)) {
            let error = format!("Esc 注销失败，面板保留可见：{error}");
            session.error = Some(error.clone()); drop(session); escape_status(app); return Err(error);
        }
        session.unregistered();
    }
    if let Err(error) = window.hide() {
        let error = format!("面板隐藏失败，全局 Esc 已释放：{error}");
        session.error = Some(error.clone()); drop(session); escape_status(app); return Err(error);
    }
    session.hidden()?; drop(session);
    let _ = window.emit("panel-visibility",serde_json::json!({"visible":false}));
    escape_status(app); Ok(())
}
fn handle_escape(app: &tauri::AppHandle, source: escape::Source, pressed: bool, repeat: bool) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let action = state.escape.lock().unwrap().signal(source,pressed,repeat,state.queue.busy());
    match action {
        escape::Action::Cancel => { state.queue.cancel(); Ok(()) }
        escape::Action::Hide => close_panel(app),
        escape::Action::Ignore => Ok(()),
    }
}
#[tauri::command]
fn panel_escape(app: tauri::AppHandle, pressed: bool, repeat: bool) -> Result<(), String> {
    handle_escape(&app,escape::Source::Local,pressed,repeat)
}
fn wake(app: &tauri::AppHandle, tab: &str, from_hotkey: bool) {
    let state = app.state::<Runtime>();
    if state.queue.busy() {
        state.log(
            "wake_rejected",
            serde_json::json!({"reason":"queue_active"}),
        );
        return;
    }
    let started = Instant::now();
    // Capture BEFORE show/focus. Own panel hotkeys preserve previous identity only.
    if !from_hotkey {
        // Persistent session: tray shows the existing explicit binding; it must
        // neither capture the tray popup nor silently replace the target.
        state.log(
            "target_preserved",
            serde_json::json!({"reason":"tray_show_existing_binding"}),
        );
    } else {
        match state.platform.capture_target() {
            Ok(target) => {
                state.log("target_captured", serde_json::json!({"beforeShow":true,"target":state.platform.target_diagnostic(&target)}));
                *state.target.lock().unwrap() = Some(target);
            }
            Err(error) if can_preserve_previous(from_hotkey, &error) => {}
            Err(error) => {
                *state.target.lock().unwrap() = None;
                state.log("target_capture_failed", serde_json::json!({"error":error}));
            }
        }
    }
    let warning = if state.target.lock().unwrap().is_none() {
        Some("请先激活目标输入框，再按 Ctrl+Alt+1；当前仅浏览，未捕获粘贴目标")
    } else {
        None
    };
    let token = state.token.fetch_add(1, Ordering::Relaxed);
    *state.wake.lock().unwrap() = Some((token, started, tab.into(), from_hotkey));
    if let Some(window) = app.get_webview_window("quick_panel") {
        let result = window.show().and_then(|_| window.set_focus());
        if !window.is_visible().unwrap_or(false) {
            state.log("panel_show_failed",serde_json::json!({"error":result.as_ref().err().map(ToString::to_string)}));
            return;
        }
        if window.is_visible().unwrap_or(false) {
            let mut session = state.escape.lock().unwrap();
            let registration = if session.registered { Ok(()) } else {
                app.global_shortcut().register(Shortcut::new(None,Code::Escape))
                    .map_err(|error|format!("全局 Esc 注册失败，当前仅浏览、不可粘贴：{error}"))
            };
            session.shown(registration); drop(session); escape_status(app);
        }
        let _ = window.emit("panel-visibility", serde_json::json!({"visible":true}));
        state.log(if from_hotkey{"hotkey"}else{"tray_browse_wake"},serde_json::json!({"token":token,"tab":tab,"windowResult":result.as_ref().err().map(ToString::to_string)}));
        let _ = window.emit(
            "wake",
            serde_json::json!({"tab":tab,"token":token,"warning":warning,"targetBound":state.target.lock().unwrap().is_some()}),
        );
    }
}
#[tauri::command]
fn get_library(state: tauri::State<Runtime>) -> data::Library {
    state.library.clone()
}
#[tauri::command]
fn get_boot(window: tauri::WebviewWindow, state: tauri::State<Runtime>) -> serde_json::Value {
    let pending = if window.label() == "quick_panel" {
        state
            .wake
            .lock()
            .unwrap()
            .as_ref()
            .map(|(token, _, tab, _)| serde_json::json!({"token":token,"tab":tab,"warning":if state.target.lock().unwrap().is_none(){Some("请先激活目标输入框，再按 Ctrl+Alt+1；当前仅浏览，未捕获粘贴目标")}else{None}}))
    } else {
        None
    };
    let session = state.escape.lock().unwrap();
    serde_json::json!({"platform":std::env::consts::OS,"logPath":state.log_path,"stage":"4.3","build":env!("CARGO_PKG_VERSION"),"revision":BUILD_REVISION,"pendingWake":pending,"panelVisible":window.is_visible().unwrap_or(false),"targetBound":state.target.lock().unwrap().is_some(),"escapeRegistered":session.registered,"escapeError":session.error})
}
#[tauri::command]
fn search_ready(
    window: tauri::WebviewWindow,
    state: tauri::State<Runtime>,
    elapsed_ms: f64,
    result_count: usize,
    sequence: u64,
    superseded: bool,
) -> Result<(), String> {
    if !elapsed_ms.is_finite() || elapsed_ms < 0.0 || result_count > state.library.cases.len() {
        return Err("invalid_search_measurement".into());
    }
    state.log(if superseded{"search_superseded"}else{"search_interactive"},serde_json::json!({"elapsedMs":elapsed_ms,"resultCount":result_count,"sequence":sequence,"window":window.label(),"cases":state.library.cases.len(),"assets":state.library.assets.len(),"measurement":"input_to_react_results_commit_and_raf","queryRecorded":false}));
    Ok(())
}
#[tauri::command]
fn ready(state: tauri::State<Runtime>, token: u64, kind: String, interactive: bool) {
    if !interactive {
        state.log("ready_not_interactive",serde_json::json!({"kind":kind,"token":token,"reason":"document_or_search_not_focused"}));
        return;
    }
    if kind == "quick" {
        let mut wake = state.wake.lock().unwrap();
        if let Some((current, start, _, from_hotkey)) = wake.as_ref() {
            if *current == token {
                state.log("quick_interactive",serde_json::json!({"token":token,"elapsedMs":start.elapsed().as_secs_f64()*1000.0,"source":if *from_hotkey{"hotkey"}else{"tray_browse"},"includesReactFocusAndRaf":true}));
                *wake = None;
            }
        }
    } else if kind == "library" {
        if let Some(start) = state.library_start.lock().unwrap().take() {
            state.log(
                "library_interactive",
                serde_json::json!({"elapsedMs":start.elapsed().as_secs_f64()*1000.0}),
            );
        }
        *state.library_creating.lock().unwrap() = false;
    }
}
#[tauri::command]
fn hide_panel(app: tauri::AppHandle) -> Result<(), String> {
    close_panel(&app)
}
#[tauri::command]
fn cancel_queue(state: tauri::State<Runtime>) -> bool {
    state.queue.cancel()
}
#[tauri::command]
fn panel_interaction(state: tauri::State<Runtime>, request_id: u64) -> bool {
    let cancelled = state.queue.cancel_request(request_id);
    if cancelled {
        state.log("panel_interaction",serde_json::json!({"requestId":request_id,"action":"cancel_pending_request_keep_panel_and_binding"}));
    }
    cancelled
}
#[tauri::command]
async fn open_library(app: tauri::AppHandle) -> Result<(), String> {
    // WebView2 creation must not run inside a synchronous command/event handler.
    tauri::async_runtime::spawn_blocking(move || create_library(&app))
        .await
        .map_err(|error| format!("library_worker_failed: {error}"))?
}
fn create_library(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Runtime>();
    if state.queue.busy() {
        return Err("queue_active".into());
    }
    if let Some(window) = app.get_webview_window("library_manager") {
        window
            .show()
            .and_then(|_| window.set_focus())
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let mut creating = state.library_creating.lock().unwrap();
    if *creating {
        return Ok(());
    }
    *creating = true;
    drop(creating);
    *state.library_start.lock().unwrap() = Some(Instant::now());
    state.log(
        "library_creating",
        serde_json::json!({"policy":"create_on_demand"}),
    );
    let result = WebviewWindowBuilder::new(
        app,
        "library_manager",
        WebviewUrl::App("index.html?window=library".into()),
    )
    .title("Prompt Shelf · 主库浏览 PoC")
    .inner_size(1360.0, 860.0)
    .min_inner_size(960.0, 580.0)
    .decorations(false)
    .build();
    if let Err(error) = result {
        *state.library_creating.lock().unwrap() = false;
        return Err(error.to_string());
    }
    Ok(())
}
#[tauri::command]
fn close_library(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("library_manager") {
        window.destroy().map_err(|e| e.to_string())?;
    }
    let state = app.state::<Runtime>();
    *state.library_creating.lock().unwrap() = false;
    state.log(
        "library_destroyed",
        serde_json::json!({"webviewProcessRelease":"requires_measurement"}),
    );
    Ok(())
}
#[tauri::command]
fn window_action(window: tauri::WebviewWindow, action: String) -> Result<(), String> {
    match action.as_str() {
        "minimize" => window.minimize(),
        "maximize" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        "close" => {
            if window.label() == "quick_panel" {
                return close_panel(window.app_handle());
            } else {
                window.destroy()
            }
        }
        _ => return Err("unknown_action".into()),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn paste(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<Runtime>,
    case_id: String,
    kind: String,
    index: Option<usize>,
    interval_ms: u64,
) -> Result<u64, String> {
    if window.label() != "quick_panel" {
        return Err("paste_only_from_quick_panel".into());
    }
    if !(50..=10000).contains(&interval_ms) {
        return Err("interval_must_be_50_to_10000_ms".into());
    }
    let target = state
        .target
        .lock()
        .unwrap()
        .clone()
        .ok_or("no_external_target_use_global_hotkey")?;
    let case = state
        .library
        .cases
        .iter()
        .find(|c| c.id == case_id)
        .ok_or("unknown_case")?;
    let image = |id: &str| {
        state
            .library
            .asset_path(id)
            .map(|path| PasteItem::Image(path, id.into()))
    };
    let items = match kind.as_str() {
        "text" => vec![PasteItem::Text(case.prompt.clone())],
        "result" => vec![image(case.result.as_deref().ok_or("no_result")?)?],
        "reference" => vec![image(
            case.refs
                .get(index.ok_or("missing_reference_index")?)
                .ok_or("reference_out_of_range")?,
        )?],
        "queue" => case
            .refs
            .iter()
            .map(|id| image(id))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("unknown_paste_kind".into()),
    };
    if items.is_empty() {
        return Err("empty_queue".into());
    }
    if !state.escape.lock().unwrap().can_paste() {
        return Err("global_escape_unavailable_no_paste: 请重新呼出面板以注册全局 Esc".into());
    }
    let ticket = state.queue.reserve()?;
    let id = ticket.id;
    let platform = state.platform.clone();
    let queue = state.queue.clone();
    state.log("request_reserved",serde_json::json!({"requestId":id,"caseId":case_id,"kind":kind,"total":items.len(),"intervalMs":interval_ms,"target":state.platform.target_diagnostic(&target)}));
    tauri::async_runtime::spawn(async move {
        queue::run(
            platform,
            target,
            ticket,
            case_id,
            items,
            Duration::from_millis(interval_ms),
            |event| {
                app.state::<Runtime>()
                    .log("queue", serde_json::to_value(&event).unwrap());
                if ["failed", "cancelled", "completed"].contains(&event.phase.as_str()) {
                    let label = if let Some(error) = &event.error {
                        format!("粘贴已停止：{error}；Ctrl+Alt+1 查看详情")
                    } else {
                        format!(
                            "{} · 已触发 {}/{}，未确认接收",
                            event.phase, event.done, event.total
                        )
                    };
                    if let Some(status) =
                        app.state::<Runtime>().tray_status.lock().unwrap().as_ref()
                    {
                        let _ = status.set_text(&label);
                    }
                    if let Some(tray) = app.tray_by_id("main") {
                        let _ = tray.set_tooltip(Some(&label));
                    }
                }
                let _ = app.emit("queue-progress", event);
            },
        )
        .await;
        queue.release();
    });
    Ok(id)
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app,shortcut,event|{
            if shortcut.matches(Modifiers::empty(),Code::Escape){let _=handle_escape(app,escape::Source::Global,event.state()==ShortcutState::Pressed,false);return;}
            if event.state()!=ShortcutState::Pressed{return;}
            for (code,tab) in [(Code::Digit1,"all"),(Code::Digit2,"recent"),(Code::Digit3,"fav")]{if shortcut.matches(Modifiers::CONTROL|Modifiers::ALT,code){wake(app,tab,true);return;}}
        }).build())
        .invoke_handler(tauri::generate_handler![get_library,get_boot,ready,search_ready,hide_panel,panel_escape,cancel_queue,panel_interaction,open_library,close_library,window_action,paste])
        .setup(|app|{
            let directory=app.path().app_local_data_dir()?.join("logs");std::fs::create_dir_all(&directory)?;
            let log_path=directory.join("poc.jsonl");let log=std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
            let library=match data::locate(){Some(path)=>data::Library::load(&path).map_err(std::io::Error::other)?,None=>data::Library::demo()};
            // Exact file allowlist; originals are never exposed over asset protocol.
            for asset in &library.assets{app.asset_protocol_scope().allow_file(&asset.thumbnail)?;}
            let platform=Arc::new(NativePlatform::default());
            #[cfg(target_os="windows")]
            if let Some(window)=app.get_webview_window("quick_panel"){platform.attach_window(&window)?;}
            let runtime=Runtime{platform,queue:Arc::new(queue::Queue::default()),escape:Mutex::new(escape::EscapeSession::default()),target:Mutex::new(None),library,log:Mutex::new(log),log_path,token:AtomicU64::new(1),wake:Mutex::new(None),library_start:Mutex::new(None),library_creating:Mutex::new(false),tray_status:Mutex::new(None)};
            runtime.log("startup",serde_json::json!({"cases":runtime.library.cases.len(),"assets":runtime.library.assets.len(),"exe":std::env::current_exe().ok(),"revision":BUILD_REVISION,"platform":std::env::consts::OS,"resourceThreshold":"baseline_only_not_approved"}));
            app.manage(runtime);
            let quick=MenuItem::with_id(app,"quick","快捷面板 · Ctrl+Alt+1",true,None::<&str>)?;
            let library=MenuItem::with_id(app,"library","主库浏览 · PoC",true,None::<&str>)?;
            let logs=MenuItem::with_id(app,"logs",format!("日志：{}",app.state::<Runtime>().log_path.display()),false,None::<&str>)?;
            let quit=MenuItem::with_id(app,"quit","退出 PoC",true,None::<&str>)?;
            let status=MenuItem::with_id(app,"status","待命 · 错误保留于此；快捷键打开查看详情",false,None::<&str>)?;
            *app.state::<Runtime>().tray_status.lock().unwrap()=Some(status.clone());
            let menu=Menu::with_items(app,&[&status,&quick,&library,&logs,&quit])?;
            let icon=tauri::image::Image::new_owned([36u8,91,255,255].repeat(32*32),32,32);
            TrayIconBuilder::with_id("main").icon(icon).tooltip("Prompt Shelf · 4.3 PoC").menu(&menu).on_menu_event(|app,event|match event.id.as_ref(){"quick"=>wake(app,"all",false),"library"=>{let app=app.clone();tauri::async_runtime::spawn(async move{if let Err(error)=open_library(app.clone()).await{app.state::<Runtime>().log("library_create_failed",serde_json::json!({"error":error}));if let Some(status)=app.state::<Runtime>().tray_status.lock().unwrap().as_ref(){let _=status.set_text(format!("主库创建失败：{error}"));}}});},"quit"=>{let cancelled=app.state::<Runtime>().queue.cancel();app.state::<Runtime>().log("exit",serde_json::json!({"cancelAccepted":cancelled,"dispatchBoundary":"cancel waits for any already-started dispatch; no new dispatch after acceptance"}));app.exit(0);},_=>{}}).build(app)?;
            for code in [Code::Digit1,Code::Digit2,Code::Digit3]{app.global_shortcut().register(Shortcut::new(Some(Modifiers::CONTROL|Modifiers::ALT),code))?;}
            if std::env::args().any(|arg|arg=="--verify-assets"){app.state::<Runtime>().library.verify_hashes().map_err(std::io::Error::other)?;app.state::<Runtime>().log("asset_hashes_verified",serde_json::json!({"count":app.state::<Runtime>().library.assets.len()}));}
            Ok(())
        })
        .on_window_event(|window,event|{
            if let tauri::WindowEvent::Focused(true)=event{if window.label()=="quick_panel"{if let Some(state)=window.try_state::<Runtime>(){if state.platform.panel_is_foreground()&&state.queue.cancel_panel_refocus(){state.log("panel_refocused",serde_json::json!({"action":"cancel_request_no_refocus_target"}));}}}}
            if let tauri::WindowEvent::CloseRequested{api,..}=event{if window.label()=="quick_panel"{api.prevent_close();let _=close_panel(window.app_handle());}}
            if let tauri::WindowEvent::Destroyed=event{if window.label()=="library_manager"{let state=window.state::<Runtime>();*state.library_creating.lock().unwrap()=false;state.log("library_window_destroyed",serde_json::json!({}));}}
        })
        .run(tauri::generate_context!()).expect("PoC startup failed: check hotkey conflicts and manifest paths");
}
#[cfg(test)]
mod wake_policy_tests {
    use super::can_preserve_previous;
    #[test]
    fn capture_errors_only_preserve_exact_quick_panel() {
        assert!(!can_preserve_previous(
            false,
            "quick_panel_keep_previous_target"
        ));
        assert!(!can_preserve_previous(
            true,
            "other_own_window_rejected_clear_target"
        ));
        assert!(can_preserve_previous(
            true,
            "quick_panel_keep_previous_target"
        ));
    }
}
