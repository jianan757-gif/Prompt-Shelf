use crate::Runtime;
use prompt_shelf_poc::{
    backup::{self, JobControl, Manifest, PreparedRestore, RestoreOutcome, Summary},
    storage_host::HostContext,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub token: String,
    pub file_name: String,
    pub created_at: i64,
    pub current: Summary,
    pub incoming: Summary,
    pub generation: u64,
    pub revision: u64,
}
struct Pending {
    preview: RestorePreview,
    prepared: PreparedRestore,
}
#[derive(Default)]
pub struct BackupJobs {
    running: Mutex<Option<Arc<JobControl>>>,
    pending: Mutex<Option<Pending>>,
    restore_notice: Mutex<Option<RestoreOutcome>>,
}
struct Job {
    jobs: Arc<BackupJobs>,
    control: Arc<JobControl>,
}
impl BackupJobs {
    fn record_restore(&self, outcome: &RestoreOutcome) {
        *self.restore_notice.lock().unwrap() = Some(outcome.clone());
    }
    fn context(&self, context: HostContext) -> RestoreOutcome {
        let notice = self.restore_notice.lock().unwrap();
        RestoreOutcome {
            context,
            restore_warning: notice
                .as_ref()
                .filter(|notice| notice.context.generation == context.generation)
                .and_then(|notice| notice.restore_warning.clone()),
        }
    }
    pub fn window_closed(&self) {
        if let Ok(running) = self.running.lock() {
            if let Some(job) = running.as_ref() {
                job.cancel();
            }
        }
        if let Ok(mut pending) = self.pending.lock() {
            pending.take();
        }
    }
    fn start(self: &Arc<Self>, id: String) -> Result<Job, String> {
        uuid::Uuid::parse_str(&id).map_err(|_| "invalid_backup_operation_id")?;
        let mut running = self.running.lock().map_err(|e| e.to_string())?;
        if running.is_some() {
            return Err("backup_job_already_running".into());
        }
        let control = Arc::new(JobControl::new(id));
        *running = Some(control.clone());
        Ok(Job {
            jobs: self.clone(),
            control,
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Ok(mut running) = self.jobs.running.lock() {
            *running = None;
        }
    }
}
fn main_only<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    if window.label() == "library_manager" {
        Ok(())
    } else {
        Err("backup_only_from_library".into())
    }
}
fn wait_queue(queue: &crate::queue::Queue, control: &JobControl) -> Result<(), String> {
    while queue.busy() {
        control.check()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    control.check()
}

#[tauri::command]
pub fn get_library_context(state: tauri::State<'_, Runtime>) -> Result<RestoreOutcome, String> {
    Ok(state.backups.context(state.host.context()?))
}
#[tauri::command]
pub async fn backup_summary(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
) -> Result<Summary, String> {
    main_only(&window)?;
    let host = app.state::<Runtime>().host.clone();
    tauri::async_runtime::spawn_blocking(move || host.read(generation, backup::summary))
        .await
        .map_err(|e| e.to_string())?
}
#[derive(Serialize)]
pub struct Exported {
    pub destination: String,
    pub manifest: Manifest,
}
#[tauri::command]
pub async fn backup_export(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    operation_id: String,
    destination: Option<String>,
    suggested_name: Option<String>,
) -> Result<Option<Exported>, String> {
    main_only(&window)?;
    let state = app.state::<Runtime>();
    let job = state.backups.start(operation_id)?;
    let host = state.host.clone();
    let queue = state.queue.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let job = job;
        let suggested = suggested_name.unwrap_or_else(|| {
            format!(
                "PromptShelf-backup-{}.zip",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            )
        });
        let destination = match destination {
            Some(path) => PathBuf::from(path),
            None => match app
                .dialog()
                .file()
                .set_parent(&window)
                .add_filter("ZIP 备份", &["zip"])
                .set_file_name(&suggested)
                .blocking_save_file()
            {
                Some(path) => path.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            },
        };
        let _maintenance = host.begin_maintenance(generation)?;
        wait_queue(&queue, &job.control)?;
        let manifest = host.read(generation, |store| {
            store.export_backup(&destination, &job.control, |progress| {
                let _ = app.emit("backup-progress", progress);
            })
        })?;
        Ok(Some(Exported {
            destination: destination.to_string_lossy().into_owned(),
            manifest,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backup_inspect(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    operation_id: String,
    source: Option<String>,
) -> Result<Option<RestorePreview>, String> {
    main_only(&window)?;
    let state = app.state::<Runtime>();
    let job = state.backups.start(operation_id)?;
    let host = state.host.clone();
    let jobs = state.backups.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let job = job;
        host.read(generation, |_| Ok(()))?;
        let source = match source {
            Some(path) => PathBuf::from(path),
            None => match app
                .dialog()
                .file()
                .set_parent(&window)
                .add_filter("ZIP 备份", &["zip"])
                .blocking_pick_file()
            {
                Some(path) => path.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            },
        };
        let prepared =
            backup::prepare_restore(&source, &host.storage_parent()?, &job.control, |progress| {
                let _ = app.emit("backup-progress", progress);
            })?;
        let (current, context) = host.observe(generation, backup::summary)?;
        let preview = RestorePreview {
            token: uuid::Uuid::new_v4().to_string(),
            file_name: source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            created_at: prepared.manifest().created_at,
            current,
            incoming: prepared.summary().clone(),
            generation: context.generation,
            revision: context.revision,
        };
        if app.get_webview_window("library_manager").is_none() {
            return Err("backup_window_closed".into());
        }
        *jobs.pending.lock().map_err(|e| e.to_string())? = Some(Pending {
            preview: preview.clone(),
            prepared,
        });
        Ok(Some(preview))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backup_refresh_preview(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    token: String,
) -> Result<RestorePreview, String> {
    main_only(&window)?;
    let state = app.state::<Runtime>();
    let host = state.host.clone();
    let jobs = state.backups.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (current, context) = host.observe(generation, backup::summary)?;
        let mut pending = jobs.pending.lock().map_err(|e| e.to_string())?;
        let pending = pending
            .as_mut()
            .filter(|p| p.preview.token == token && p.preview.generation == generation)
            .ok_or("restore_preview_expired")?;
        pending.preview.current = current;
        pending.preview.revision = context.revision;
        Ok(pending.preview.clone())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn backup_restore(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    token: String,
    expected_revision: u64,
    operation_id: String,
) -> Result<RestoreOutcome, String> {
    main_only(&window)?;
    let state = app.state::<Runtime>();
    let job = state.backups.start(operation_id)?;
    let host = state.host.clone();
    let jobs = state.backups.clone();
    let queue = state.queue.clone();
    let edits = state.edits.clone();
    let grants = state.thumbnails.clone();
    if state
        .closing
        .lock()
        .map_err(|e| e.to_string())?
        .pending
        .is_some()
    {
        return Err("library_close_pending".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let job = job;
        let mut maintenance = host.begin_maintenance(generation)?;
        wait_queue(&queue, &job.control)?;
        edits.prepare_close()?;
        job.control.check()?;
        let prepared = {
            let mut pending = jobs.pending.lock().map_err(|e| e.to_string())?;
            let p = pending
                .as_ref()
                .filter(|p| p.preview.token == token && p.preview.generation == generation)
                .ok_or("restore_preview_expired")?;
            if p.preview.revision != expected_revision || host.revision() != expected_revision {
                return Err("restore_preview_changed_reconfirm".into());
            }
            pending.take().unwrap().prepared
        };
        job.control.begin_commit()?;
        let _ = app.emit(
            "backup-progress",
            backup::Progress {
                operation_id: job.control.operation_id().to_owned(),
                phase: "committing".into(),
                bytes_done: 0,
                bytes_total: 0,
                files_done: 0,
                files_total: 0,
            },
        );
        let path = prepared.into_storage()?;
        let context = maintenance.install(&path, expected_revision)?;
        // Complete fallible data initialization before revoking old grants: forbid_file
        // takes precedence over allow_file and must not strand a rollback's old thumbnails.
        let initialized = host.read(context.generation, |store| {
            store.library_index()?;
            store.draft_summaries()?;
            Ok(())
        });
        if let Err(error) = initialized {
            let rollback = maintenance.rollback()?;
            let _ = app.emit("library-reinitialized", rollback);
            return Err(format!(
                "restore_initialization_failed_rolled_back: {error}"
            ));
        }
        let outcome = backup::restore_outcome(
            context,
            grants.revoke_previous(&app, &path),
            |kind, data| {
                app.state::<Runtime>().log(kind, data);
            },
        );
        jobs.record_restore(&outcome);
        let _ = app.emit("library-reinitialized", &outcome);
        Ok(outcome)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn backup_cancel(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    operation_id: String,
) -> Result<(), String> {
    main_only(&window)?;
    let state = app.state::<Runtime>();
    let job = state.backups.running.lock().map_err(|e| e.to_string())?;
    if let Some(job) = job.as_ref().filter(|j| j.operation_id() == operation_id) {
        if !job.cancel() {
            return Err("backup_commit_already_started".into());
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn backup_release_preview(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    generation: u64,
    token: String,
) -> Result<(), String> {
    main_only(&window)?;
    let state = app.state::<Runtime>();
    let host = state.host.clone();
    let jobs = state.backups.clone();
    tauri::async_runtime::spawn_blocking(move || {
        host.read(generation, |_| {
            let mut pending = jobs.pending.lock().map_err(|e| e.to_string())?;
            if pending.as_ref().is_some_and(|p| p.preview.token == token) {
                pending.take();
            }
            Ok(())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use prompt_shelf_poc::storage_host::LibraryHost;
    #[test]
    fn committed_restore_keeps_success_and_replays_cleanup_warning_with_log() {
        use prompt_shelf_poc::{paths::DataPaths, store::Store};
        use std::io::Write;
        for failed_cleanup in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let host = LibraryHost::open(root.path()).unwrap();
            let path = host
                .storage_parent()
                .unwrap()
                .join(uuid::Uuid::new_v4().to_string());
            let incoming = Store::open(DataPaths::new(&path).unwrap()).unwrap();
            let mut document = incoming.edit_document(None).unwrap();
            document.title = "restored library".into();
            document.prompt = "restored content".into();
            incoming.commit_case(&document).unwrap();
            drop(incoming);
            let mut maintenance = host.begin_maintenance(1).unwrap();
            let context = maintenance.install(&path, host.revision()).unwrap();
            host.read(context.generation, |s| s.library_index())
                .unwrap();
            let log_path = root.path().join("test-log.jsonl");
            let mut log = std::fs::File::create(&log_path).unwrap();
            let cleanup = if failed_cleanup {
                Err("injected_scope_cleanup_failure".to_owned())
            } else {
                Ok(())
            };
            let outcome = backup::restore_outcome(context, cleanup, |kind, data| {
                writeln!(log, "{}", serde_json::json!({"kind":kind,"data":data})).unwrap()
            });
            let jobs = BackupJobs::default();
            jobs.record_restore(&outcome);
            drop(maintenance);
            drop(log);
            assert_eq!(outcome.context.generation, 2);
            assert_eq!(
                host.read(2, |s| Ok(s.library_index()?.cases[0].title.clone()))
                    .unwrap(),
                "restored library"
            );
            let replay = jobs.context(host.context().unwrap());
            assert_eq!(replay.restore_warning.is_some(), failed_cleanup);
            let logs = std::fs::read_to_string(log_path).unwrap();
            if failed_cleanup {
                let record: serde_json::Value = serde_json::from_str(logs.trim()).unwrap();
                assert_eq!(record["kind"], "restore_scope_cleanup_warning");
                assert_eq!(record["data"]["generation"], 2);
                assert_eq!(record["data"]["error"], "injected_scope_cleanup_failure");
                assert!(outcome.restore_warning.unwrap().contains("已恢复成功"));
            } else {
                assert!(logs.is_empty());
            }
        }
    }
    #[tokio::test]
    async fn dropped_waiter_and_cancel_do_not_release_job_or_maintenance_before_worker_ends() {
        let directory = tempfile::tempdir().unwrap();
        let host = Arc::new(LibraryHost::open(directory.path()).unwrap());
        let jobs = Arc::new(BackupJobs::default());
        let job = jobs.start(uuid::Uuid::new_v4().to_string()).unwrap();
        let control = job.control.clone();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker_host = host.clone();
        let waiter = tauri::async_runtime::spawn_blocking(move || {
            let job = job;
            {
                let _maintenance = worker_host.begin_maintenance(1).unwrap();
                entered_tx.send(()).unwrap();
                finish_rx.recv().unwrap();
                assert!(job.control.check().is_err());
            }
            drop(job);
            done_tx.send(()).unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(control.cancel());
        drop(waiter);
        assert!(jobs.start(uuid::Uuid::new_v4().to_string()).is_err());
        assert!(host.write(1, |_| Ok(())).is_err());
        assert!(host.read(1, |s| s.counts()).is_ok());
        finish_tx.send(()).unwrap();
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(host.write(1, |_| Ok(())).is_ok());
        assert!(jobs.start(uuid::Uuid::new_v4().to_string()).is_ok());
    }
    #[test]
    fn maintenance_waits_for_reserved_queue_and_allows_only_ongoing_writes() {
        let directory = tempfile::tempdir().unwrap();
        let host = Arc::new(LibraryHost::open(directory.path()).unwrap());
        let queue = Arc::new(crate::queue::Queue::default());
        let _ticket = queue.reserve().unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let h = host.clone();
        let q = queue.clone();
        let worker = std::thread::spawn(move || {
            let _maintenance = h.begin_maintenance(1).unwrap();
            entered_tx.send(()).unwrap();
            wait_queue(&q, &JobControl::new("test")).unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(host.write(1, |_| Ok(())).is_err());
        host.ongoing_write(1, |_| Ok(())).unwrap();
        queue.release();
        worker.join().unwrap();
        host.write(1, |_| Ok(())).unwrap();
    }
}
