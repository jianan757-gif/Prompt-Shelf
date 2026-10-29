//! Headless-browser IPC bridge for integration tests. Never opens a default/user library.
use prompt_shelf_poc::{
    backup::{self, JobControl, PreparedRestore, RestoreOutcome, Summary},
    cases::{CaseDocument, EditSessions, ImageReference},
    storage_host::{HostContext, LibraryHost},
    store::Store,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
fn arg<'a>(r: &'a Value, key: &str) -> Result<&'a str, String> {
    r["args"][key].as_str().ok_or(format!("missing {key}"))
}
fn call_on_store(s: &Store, e: &EditSessions, r: &Value, root: &Path) -> Result<Value, String> {
    Ok(match r["command"].as_str().unwrap_or("") {
        "get_library_index" => serde_json::to_value(s.library_index()?).unwrap(),
        "get_case_assets" => serde_json::to_value(
            s.assets_for_cases(
                &serde_json::from_value::<Vec<String>>(r["args"]["caseIds"].clone())
                    .map_err(|e| e.to_string())?,
            )?,
        )
        .unwrap(),
        "get_trash_index" => serde_json::to_value(s.trash_index()?).unwrap(),
        "get_trash_assets" => serde_json::to_value(
            s.trash_assets(
                &serde_json::from_value::<Vec<String>>(r["args"]["caseIds"].clone())
                    .map_err(|e| e.to_string())?,
            )?,
        )
        .unwrap(),
        "get_case_delete_draft" => json!(s.case_delete_draft(arg(r, "caseId")?)?),
        "soft_delete_case" => {
            e.while_idle(|| s.soft_delete_case(arg(r, "caseId")?))?;
            Value::Null
        }
        "restore_case" => {
            e.while_idle(|| s.restore_case(arg(r, "caseId")?))?;
            Value::Null
        }
        "permanently_delete_case" => {
            e.while_idle(|| s.permanently_delete_case(arg(r, "caseId")?))?;
            Value::Null
        }
        "discard_case_draft" => {
            e.discard_unopened(s, arg(r, "draftId")?)?;
            Value::Null
        }
        "test_suspend_editor" => {
            e.clear();
            Value::Null
        }
        // Only this marked temp-directory bridge exposes anomaly injection; production has no such IPC.
        "test_create_related_draft" => {
            let c = rusqlite::Connection::open(s.paths().database()?).map_err(|e| e.to_string())?;
            c.pragma_update(None, "foreign_keys", true)
                .map_err(|e| e.to_string())?;
            let draft = uuid::Uuid::new_v4().to_string();
            let count=c.execute("INSERT INTO drafts(id,case_id,base_revision,title,prompt,project_id,updated_at,dirty) SELECT ?1,id,edit_revision,title,prompt||' related draft',project_id,1,?3 FROM cases WHERE id=?2",rusqlite::params![draft,arg(r,"caseId")?,r["args"]["dirty"].as_bool().unwrap_or(true)]).map_err(|e|e.to_string())?;
            if count != 1 {
                return Err("test_case_missing".into());
            }
            json!(draft)
        }
        "catalog_create" => {
            let id = e.while_idle(|| s.catalog_create(arg(r, "kind")?, arg(r, "name")?))?;
            json!(id)
        }
        "catalog_rename" => {
            e.while_idle(|| s.catalog_rename(arg(r, "kind")?, arg(r, "id")?, arg(r, "name")?))?;
            Value::Null
        }
        "catalog_delete" => {
            let ids = serde_json::from_value::<Vec<String>>(r["args"]["ids"].clone())
                .map_err(|e| e.to_string())?;
            e.while_idle(|| s.catalog_delete(arg(r, "kind")?, &ids))?;
            Value::Null
        }
        "set_quick_tag" => {
            e.while_idle(|| {
                s.set_quick_tag(arg(r, "id")?, r["args"]["enabled"].as_bool().unwrap())
            })?;
            Value::Null
        }
        "reorder_quick_tags" => {
            let ids = serde_json::from_value::<Vec<String>>(r["args"]["ids"].clone())
                .map_err(|e| e.to_string())?;
            e.while_idle(|| s.reorder_quick_tags(&ids))?;
            Value::Null
        }
        "set_favorite" => {
            s.set_favorite(arg(r, "caseId")?, r["args"]["favorite"].as_bool().unwrap())?;
            Value::Null
        }
        "set_ui_preferences" => {
            s.set_ui_preferences(r["args"]["patch"].clone())?;
            Value::Null
        }
        "save_dictionary" => {
            s.save_dictionary(
                serde_json::from_value::<Vec<Vec<String>>>(r["args"]["groups"].clone())
                    .map_err(|e| e.to_string())?,
            )?;
            Value::Null
        }
        "test_record_use" => {
            s.record_use(arg(r, "caseId")?)?;
            Value::Null
        }
        "begin_case_edit" => serde_json::to_value(e.begin_persistent(
            s,
            r["args"]["caseId"].as_str(),
            None,
            false,
            e.epoch(),
        )?)
        .unwrap(),
        "save_case_draft" => json!(e.stash(
            s,
            arg(r, "sessionId")?,
            &serde_json::from_value::<CaseDocument>(r["args"]["document"].clone())
                .map_err(|e| e.to_string())?,
            r["args"]["saveSeq"].as_u64().ok_or("seq")?
        )?),
        "commit_case" => serde_json::to_value(
            e.save(
                s,
                arg(r, "sessionId")?,
                &serde_json::from_value::<CaseDocument>(r["args"]["document"].clone())
                    .map_err(|e| e.to_string())?,
            )?,
        )
        .unwrap(),
        "cancel_case_edit" => {
            e.discard(s, arg(r, "sessionId")?)?;
            Value::Null
        }
        "recover_case_draft" => serde_json::to_value(e.begin_persistent(
            s,
            None,
            Some(arg(r, "draftId")?),
            r["args"]["asNew"].as_bool().unwrap_or(false),
            e.epoch(),
        )?)
        .unwrap(),
        "list_case_drafts" => serde_json::to_value(s.draft_summaries()?).unwrap(),
        "get_library" => serde_json::to_value(s.library()?).unwrap(),
        "normalize_source_url" => {
            json!(prompt_shelf_poc::cases::normalize_source(arg(r, "value")?)?)
        }
        "import_case_images" => {
            e.check(arg(r, "sessionId")?)?;
            let files = r["args"]["paths"]
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![json!(root.join("test-import.png").to_string_lossy())]);
            let mut output = vec![];
            for file in files {
                let path = std::path::Path::new(file.as_str().ok_or("path")?);
                let asset = s.import_image(path)?;
                let mut value = serde_json::to_value(asset).unwrap();
                value["originalName"] = json!(path.file_name().unwrap().to_string_lossy());
                output.push(value)
            }
            e.check(arg(r, "sessionId")?)?;
            json!(output)
        }
        "probe" => {
            let drafts = s.draft_summaries()?;
            let documents = drafts
                .iter()
                .map(|d| {
                    let (document, seq) = s.draft_document(&d.id).unwrap();
                    json!({"draftId":d.id,"document":document,"seq":seq})
                })
                .collect::<Vec<_>>();
            json!({"library":s.library()?,"drafts":drafts,"documents":documents})
        }
        _ => return Err("unsupported_test_command".into()),
    })
}
fn seed(store: &Store, root: &Path) {
    if !root.join("browser-test-seeded").is_file() {
        let mut assets = vec![];
        for (index, (width, height)) in [(12, 8), (8, 12), (10, 10), (13, 10)]
            .into_iter()
            .enumerate()
        {
            let file = root.join(format!("test-{index}.png"));
            image::RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([40 + index as u8 * 20, 90, 120, 160]),
            )
            .save(&file)
            .unwrap();
            assets.push(store.import_image(&file).unwrap().id)
        }
        image::RgbaImage::from_pixel(15, 10, image::Rgba([160, 80, 40, 120]))
            .save(root.join("test-import.png"))
            .unwrap();
        let mut d = store.edit_document(None).unwrap();
        d.title = "正式 A".into();
        d.prompt = "正式提示词 A".into();
        d.result = Some(ImageReference {
            id: uuid::Uuid::new_v4().to_string(),
            asset_id: assets[0].clone(),
            original_name: "效果.png".into(),
        });
        d.references = assets
            .iter()
            .enumerate()
            .map(|(i, asset)| ImageReference {
                id: uuid::Uuid::new_v4().to_string(),
                asset_id: asset.clone(),
                original_name: format!("参考{i}.png"),
            })
            .collect();
        store.commit_case(&d).unwrap();
        let mut b = store.edit_document(None).unwrap();
        b.title = "共享案例 B".into();
        b.prompt = "正式 B".into();
        b.references = vec![ImageReference {
            id: uuid::Uuid::new_v4().to_string(),
            asset_id: assets[0].clone(),
            original_name: "共享.png".into(),
        }];
        store.commit_case(&b).unwrap();
        std::fs::write(
            root.join("browser-test-seeded"),
            "synthetic fixture seeded once",
        )
        .unwrap();
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Preview {
    token: String,
    file_name: String,
    created_at: i64,
    current: Summary,
    incoming: Summary,
    generation: u64,
    revision: u64,
}
struct Pending {
    preview: Preview,
    prepared: PreparedRestore,
}
#[derive(Default)]
struct Jobs {
    running: Mutex<Option<Arc<JobControl>>>,
    pending: Mutex<Option<Pending>>,
    restore_notice: Mutex<Option<RestoreOutcome>>,
}
struct Lease {
    jobs: Arc<Jobs>,
    control: Arc<JobControl>,
}
impl Jobs {
    fn start(self: &Arc<Self>, id: &str) -> Result<Lease, String> {
        uuid::Uuid::parse_str(id).map_err(|_| "invalid_backup_operation_id")?;
        let mut current = self.running.lock().map_err(|e| e.to_string())?;
        if current.is_some() {
            return Err("backup_job_already_running".into());
        }
        let control = Arc::new(JobControl::new(id));
        *current = Some(control.clone());
        Ok(Lease {
            jobs: self.clone(),
            control,
        })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut running) = self.jobs.running.lock() {
            *running = None;
        }
    }
}
struct Bridge {
    host: Arc<LibraryHost>,
    edits: Arc<EditSessions>,
    jobs: Arc<Jobs>,
    root: PathBuf,
    restore_path: Mutex<PathBuf>,
    pause_phase: Mutex<Option<String>>,
    fail_scope_cleanup: std::sync::atomic::AtomicBool,
    output: Mutex<std::io::Stdout>,
}
impl Bridge {
    fn output(&self, value: Value) {
        let mut output = self.output.lock().unwrap();
        writeln!(output, "BRIDGE {value}").unwrap();
        output.flush().unwrap();
    }
    fn reply(&self, request: &Value, result: Result<Value, String>) {
        self.output(match result {
            Ok(value) => json!({"id":request["id"],"value":value}),
            Err(error) => json!({"id":request["id"],"error":error}),
        })
    }
    fn event(&self, event: &str, payload: impl serde::Serialize) {
        self.output(json!({"event":event,"payload":payload}));
    }
    fn progress(&self, control: &JobControl, event: backup::Progress) {
        let phase = event.phase.clone();
        self.event("backup-progress", event);
        // This test scheduling barrier makes cancellation of a tiny synthetic ZIP observable.
        while control.check().is_ok()
            && self.pause_phase.lock().unwrap().as_deref() == Some(phase.as_str())
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    fn generation(&self, request: &Value) -> Result<u64, String> {
        match request["args"].get("generation") {
            None => Ok(self.host.context()?.generation),
            Some(value) => value.as_u64().ok_or("invalid_library_generation".into()),
        }
    }
    fn selected_path(
        &self,
        request: &Value,
        key: &str,
        default: PathBuf,
        existing: bool,
    ) -> Result<PathBuf, String> {
        let path = request["args"][key]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or(default);
        let checked = if existing {
            dunce::canonicalize(&path).map_err(|e| e.to_string())?
        } else {
            let parent = dunce::canonicalize(path.parent().ok_or("test_path_parent_missing")?)
                .map_err(|e| e.to_string())?;
            parent.join(path.file_name().ok_or("test_path_name_missing")?)
        };
        if !checked.starts_with(&self.root) {
            return Err("test_path_outside_owned_root".into());
        }
        Ok(checked)
    }
    fn call(&self, request: &Value) -> Result<Value, String> {
        let command = request["command"].as_str().unwrap_or("");
        if command == "get_library_context" {
            let context = self.host.context()?;
            let notice = self.jobs.restore_notice.lock().unwrap();
            return Ok(json!(RestoreOutcome {
                context,
                restore_warning: notice
                    .as_ref()
                    .filter(|n| n.context.generation == context.generation)
                    .and_then(|n| n.restore_warning.clone())
            }));
        }
        if command == "test_fail_scope_cleanup" {
            self.fail_scope_cleanup.store(
                request["args"]["enabled"].as_bool().unwrap(),
                std::sync::atomic::Ordering::Release,
            );
            return Ok(Value::Null);
        }
        if command == "backup_cancel" {
            let current = self.jobs.running.lock().map_err(|e| e.to_string())?;
            if let Some(control) = current
                .as_ref()
                .filter(|c| c.operation_id() == arg(request, "operationId").unwrap_or(""))
            {
                if !control.cancel() {
                    return Err("backup_commit_already_started".into());
                }
            }
            return Ok(Value::Null);
        }
        if command == "test_set_restore_path" {
            let path =
                self.selected_path(request, "path", self.root.join("browser-backup.zip"), true)?;
            *self.restore_path.lock().map_err(|e| e.to_string())? = path;
            return Ok(Value::Null);
        }
        if command == "test_pause_backup" {
            *self.pause_phase.lock().map_err(|e| e.to_string())? =
                request["args"]["phase"].as_str().map(str::to_owned);
            return Ok(Value::Null);
        }
        if command == "test_resume_backup" {
            *self.pause_phase.lock().map_err(|e| e.to_string())? = None;
            return Ok(Value::Null);
        }
        let generation = self.generation(request)?;
        if command == "test_pending_path" {
            return self.host.read(generation, |_| {
                Ok(json!(self
                    .jobs
                    .pending
                    .lock()
                    .map_err(|e| e.to_string())?
                    .as_ref()
                    .map(|p| p.prepared.path().to_string_lossy().into_owned())))
            });
        }
        if command == "backup_release_preview" {
            return self.host.read(generation, |_| {
                let mut pending = self.jobs.pending.lock().map_err(|e| e.to_string())?;
                if pending.as_ref().is_some_and(|p| {
                    p.preview.token == arg(request, "token").unwrap_or("")
                        && p.preview.generation == generation
                }) {
                    pending.take();
                }
                Ok(Value::Null)
            });
        }
        if command == "test_paths" {
            return self.host.read(generation,|s|Ok(json!({"root":self.root,"backupPath":self.root.join("browser-backup.zip"),"restorePath":&*self.restore_path.lock().map_err(|e|e.to_string())?,"currentStorage":s.paths().root()})));
        }
        if command == "backup_summary" {
            return self
                .host
                .read(generation, |s| Ok(json!(backup::summary(s)?)));
        }
        if command == "backup_refresh_preview" {
            let (current, context) = self.host.observe(generation, backup::summary)?;
            let mut pending = self.jobs.pending.lock().map_err(|e| e.to_string())?;
            let pending = pending
                .as_mut()
                .filter(|p| {
                    p.preview.token == arg(request, "token").unwrap_or("")
                        && p.preview.generation == generation
                })
                .ok_or("restore_preview_expired")?;
            pending.preview.current = current;
            pending.preview.revision = context.revision;
            return Ok(json!(pending.preview));
        }
        let action = |store: &Store| call_on_store(store, &self.edits, request, &self.root);
        if [
            "get_library_index",
            "get_case_assets",
            "get_trash_index",
            "get_trash_assets",
            "get_case_delete_draft",
            "list_case_drafts",
            "get_library",
            "normalize_source_url",
            "probe",
        ]
        .contains(&command)
        {
            self.host.read(generation, action)
        } else {
            self.host.write(generation, action)
        }
    }
    fn backup(&self, request: &Value, job: &Lease) -> Result<Value, String> {
        let generation = self.generation(request)?;
        match request["command"].as_str().unwrap_or("") {
            "backup_export" => {
                let destination = self.selected_path(
                    request,
                    "destination",
                    self.root.join("browser-backup.zip"),
                    false,
                )?;
                let _maintenance = self.host.begin_maintenance(generation)?;
                job.control.check()?;
                let manifest = self.host.read(generation, |s| {
                    s.export_backup(&destination, &job.control, |event| {
                        self.progress(&job.control, event)
                    })
                })?;
                Ok(json!({"destination":destination,"manifest":manifest}))
            }
            "backup_inspect" => {
                self.host.read(generation, |_| Ok(()))?;
                let default = self.restore_path.lock().map_err(|e| e.to_string())?.clone();
                let source = self.selected_path(request, "source", default, true)?;
                let prepared = backup::prepare_restore(
                    &source,
                    &self.host.storage_parent()?,
                    &job.control,
                    |event| self.progress(&job.control, event),
                )?;
                let (current, context) = self.host.observe(generation, backup::summary)?;
                let preview = Preview {
                    token: uuid::Uuid::new_v4().to_string(),
                    file_name: source.file_name().unwrap().to_string_lossy().into_owned(),
                    created_at: prepared.manifest().created_at,
                    current,
                    incoming: prepared.summary().clone(),
                    generation: context.generation,
                    revision: context.revision,
                };
                *self.jobs.pending.lock().map_err(|e| e.to_string())? = Some(Pending {
                    preview: preview.clone(),
                    prepared,
                });
                Ok(json!(preview))
            }
            "backup_restore" => {
                let mut maintenance = self.host.begin_maintenance(generation)?;
                self.edits.prepare_close()?;
                job.control.check()?;
                let revision = request["args"]["expectedRevision"]
                    .as_u64()
                    .ok_or("expected_revision_missing")?;
                let prepared = {
                    let mut pending = self.jobs.pending.lock().map_err(|e| e.to_string())?;
                    let item = pending
                        .as_ref()
                        .filter(|p| {
                            p.preview.token == arg(request, "token").unwrap_or("")
                                && p.preview.generation == generation
                        })
                        .ok_or("restore_preview_expired")?;
                    if item.preview.revision != revision || self.host.revision() != revision {
                        return Err("restore_preview_changed_reconfirm".into());
                    }
                    pending.take().unwrap().prepared
                };
                let path = prepared.into_storage()?;
                job.control.begin_commit()?;
                self.event(
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
                let context: HostContext = maintenance.install(&path, revision)?;
                let initialized = self.host.read(context.generation, |s| {
                    s.library_index()?;
                    s.draft_summaries()?;
                    Ok(())
                });
                if let Err(error) = initialized {
                    let previous = maintenance.rollback()?;
                    self.event("library-reinitialized", previous);
                    return Err(format!(
                        "restore_initialization_failed_rolled_back: {error}"
                    ));
                }
                // Expose the new generation first, modelling a focus read before cleanup
                // completes. The later warning must reach an already-remounted window.
                self.event("library-reinitialized", context);
                let cleanup = if self
                    .fail_scope_cleanup
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    Err("injected_scope_cleanup_failure".to_owned())
                } else {
                    Ok(())
                };
                let outcome = backup::restore_outcome(context, cleanup, |kind, data| {
                    self.event("test-log", json!({"kind":kind,"data":data}))
                });
                *self.jobs.restore_notice.lock().unwrap() = Some(outcome.clone());
                self.event("library-reinitialized", &outcome);
                Ok(json!(outcome))
            }
            _ => Err("unsupported_backup_test_command".into()),
        }
    }
}

#[test]
fn browser_store_bridge() {
    let Some(root) = std::env::var_os("PROMPT_SHELF_BROWSER_TEST_DIR") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.join("browser-test-owned").is_file(),
        "Only a test-owned directory may be opened"
    );
    let host = Arc::new(LibraryHost::open(&root).unwrap());
    let root = host.root().to_owned();
    let generation = host.context().unwrap().generation;
    host.write(generation, |store| {
        seed(store, &root);
        Ok(())
    })
    .unwrap();
    let bridge = Arc::new(Bridge {
        host,
        edits: Arc::new(EditSessions::default()),
        jobs: Arc::new(Jobs::default()),
        restore_path: Mutex::new(root.join("browser-backup.zip")),
        pause_phase: Mutex::new(None),
        fail_scope_cleanup: std::sync::atomic::AtomicBool::new(false),
        root,
        output: Mutex::new(std::io::stdout()),
    });
    bridge.output(json!({"ready":true}));
    let mut workers = vec![];
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        if ["backup_export", "backup_inspect", "backup_restore"]
            .contains(&request["command"].as_str().unwrap_or(""))
        {
            match bridge
                .jobs
                .start(arg(&request, "operationId").unwrap_or(""))
            {
                Ok(job) => {
                    let worker = bridge.clone();
                    workers.push(std::thread::spawn(move || {
                        let result = worker.backup(&request, &job);
                        drop(job);
                        worker.reply(&request, result);
                    }));
                }
                Err(error) => bridge.reply(&request, Err(error)),
            }
        } else {
            let result = bridge.call(&request);
            bridge.reply(&request, result);
        }
    }
    for worker in workers {
        worker.join().unwrap();
    }
}
