use super::*;
use crate::cases::{EditSessions, ImageReference};
use std::{
    process::{Command, Stdio},
    sync::{mpsc, Arc, Barrier},
    time::{Duration, Instant},
};

fn title(store: &Store) -> String {
    store.library_index().unwrap().cases[0].title.clone()
}
fn add_case(store: &Store, title: &str) -> String {
    let mut document = store.edit_document(None).unwrap();
    document.title = title.into();
    document.prompt = format!("prompt {title}");
    store.commit_case(&document).unwrap().id
}
fn prepare(host: &LibraryHost, title: &str) -> PathBuf {
    let path = host
        .storage_parent()
        .unwrap()
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::open(DataPaths::new(&path).unwrap()).unwrap();
    add_case(&store, title);
    drop(store);
    path
}
fn host(root: &Path) -> LibraryHost {
    LibraryHost::open(root).unwrap()
}
fn context(host: &LibraryHost) -> HostContext {
    host.context().unwrap()
}

#[test]
fn root_lease_excludes_second_entry_and_flat_library_is_never_moved() {
    let root = tempfile::tempdir().unwrap();
    let old = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
    add_case(&old, "flat");
    drop(old);
    let active = host(root.path());
    assert_eq!(active.read(1, |s| Ok(title(s))).unwrap(), "flat");
    assert!(LibraryHost::open(root.path())
        .err()
        .unwrap()
        .contains("library_in_use"));
    assert!(root.path().join("library.db").is_file());
    assert!(!root.path().join("active.json").exists());
    drop(active);
    assert_eq!(host(root.path()).read(1, |s| Ok(title(s))).unwrap(), "flat");
}

#[test]
fn generations_maintenance_and_revision_gate_all_operations() {
    let root = tempfile::tempdir().unwrap();
    let active = host(root.path());
    assert_eq!(active.revision(), 0);
    active
        .write(1, |s| {
            add_case(s, "old");
            Ok(())
        })
        .unwrap();
    assert_eq!(active.revision(), 1);
    assert!(active.write(1, |_| Err::<(), _>("failure".into())).is_err());
    assert_eq!(active.revision(), 1);
    let mut maintenance = active.begin_maintenance(1).unwrap();
    assert!(active.begin_maintenance(1).is_err());
    assert!(active
        .write(1, |_| Ok(()))
        .err()
        .unwrap()
        .contains("maintenance"));
    assert!(active.read(1, |s| Ok(title(s))).is_ok());
    active.ongoing_write(1, |_| Ok(())).unwrap();
    assert_eq!(active.revision(), 2);
    let path = prepare(&active, "new");
    assert_eq!(
        maintenance.install(&path, 1).err().unwrap(),
        "restore_preview_changed_reconfirm"
    );
    let installed = maintenance.install(&path, 2).unwrap();
    assert_eq!(installed.generation, 2);
    assert!(installed.maintenance);
    for result in [
        active.read(1, |_| Ok(())),
        active.write(1, |_| Ok(())),
        active.ongoing_write(1, |_| Ok(())),
    ] {
        assert_eq!(result.unwrap_err(), "stale_library_generation");
    }
    drop(maintenance);
    assert!(!context(&active).maintenance);
    active.write(2, |_| Ok(())).unwrap();
    assert_eq!(active.read(2, |s| Ok(title(s))).unwrap(), "new");
}

#[test]
fn switch_closes_old_database_but_retains_originals_drafts_and_flat_rollback() {
    let root = tempfile::tempdir().unwrap();
    let sources = tempfile::tempdir().unwrap();
    let active = host(root.path());
    let sessions = EditSessions::default();
    let (case_id, original) = active
        .write(1, |s| {
            let source = sources.path().join("original.png");
            image::RgbaImage::from_pixel(8, 6, image::Rgba([50, 90, 160, 255]))
                .save(&source)
                .unwrap();
            let asset = s.import_image(&source)?;
            let mut d = s.edit_document(None)?;
            d.title = "old".into();
            d.prompt = "formal".into();
            d.references = vec![ImageReference {
                id: uuid::Uuid::new_v4().to_string(),
                asset_id: asset.id.clone(),
                original_name: "original.png".into(),
            }];
            s.commit_case(&d)?;
            s.output_snapshot_at_revision(&d.id, 1)?;
            let receipt =
                sessions.begin_persistent(s, Some(&d.id), None, false, sessions.epoch())?;
            let mut draft = receipt.document;
            draft.prompt = "old draft survives".into();
            sessions.stash(s, &receipt.session_id, &draft, 1)?;
            Ok((d.id, PathBuf::from(asset.original)))
        })
        .unwrap();
    sessions.clear();
    let new_path = prepare(&active, "new");
    let revision = active.revision();
    let mut maintenance = active.begin_maintenance(1).unwrap();
    maintenance.install(&new_path, revision).unwrap();
    assert!(original.is_file());
    assert!(root.path().join("library.db").is_file());
    // Opening the old Store proves its DB handle/local lease was released by the switch.
    let old = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
    assert_eq!(
        old.draft_summaries().unwrap()[0].prompt,
        "old draft survives"
    );
    assert_eq!(old.edit_document(Some(&case_id)).unwrap().prompt, "formal");
    drop(old);
    let reverted = maintenance.rollback().unwrap();
    assert_eq!(reverted.generation, 3);
    assert_eq!(active.read(3, |s| Ok(title(s))).unwrap(), "old");
    drop(maintenance);
    drop(active);
    let reopened = host(root.path());
    assert_eq!(context(&reopened).generation, 3);
    assert_eq!(
        reopened
            .read(3, |s| Ok(s.draft_summaries()?.len()))
            .unwrap(),
        1
    );
    assert!(new_path.join("library.db").is_file());
}

#[test]
fn ordinary_errors_at_both_pointer_checkpoints_leave_runtime_and_disk_on_old_library() {
    for fault in [SwitchPoint::PreviousDurable, SwitchPoint::ActiveDurable] {
        let root = tempfile::tempdir().unwrap();
        let active = host(root.path());
        active
            .write(1, |s| {
                add_case(s, "old");
                Ok(())
            })
            .unwrap();
        let new_path = prepare(&active, "new");
        let revision = active.revision();
        let mut maintenance = active.begin_maintenance(1).unwrap();
        let result = maintenance.install_inner(&new_path, revision, |point| {
            if std::mem::discriminant(&fault) == std::mem::discriminant(&point) {
                Err("injected_pointer_stage_failure".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.err().unwrap(), "injected_pointer_stage_failure");
        assert_eq!(active.read(1, |s| Ok(title(s))).unwrap(), "old");
        drop(maintenance);
        drop(active);
        let reopened = host(root.path());
        let generation = context(&reopened).generation;
        assert_eq!(reopened.read(generation, |s| Ok(title(s))).unwrap(), "old");
        assert!(new_path.join("library.db").is_file());
    }
}

#[test]
fn bad_active_falls_back_to_verified_previous_with_new_generation_and_bad_both_never_create_empty()
{
    let root = tempfile::tempdir().unwrap();
    let active = host(root.path());
    active
        .write(1, |s| {
            add_case(s, "old");
            Ok(())
        })
        .unwrap();
    let next = prepare(&active, "new");
    let revision = active.revision();
    let mut maintenance = active.begin_maintenance(1).unwrap();
    maintenance.install(&next, revision).unwrap();
    drop(maintenance);
    drop(active);
    // A valid pointer to a missing database must not create a replacement database.
    std::fs::remove_file(next.join("library.db")).unwrap();
    let fallback = host(root.path());
    assert_eq!(context(&fallback).generation, 3);
    assert_eq!(fallback.read(3, |s| Ok(title(s))).unwrap(), "old");
    assert!(!next.join("library.db").exists());
    drop(fallback);
    std::fs::write(root.path().join("active.json"), b"bad json").unwrap();
    std::fs::write(root.path().join("active.previous.json"), b"bad previous").unwrap();
    assert!(LibraryHost::open(root.path())
        .err()
        .unwrap()
        .contains("library_recovery_required"));
    let old = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
    assert_eq!(title(&old), "old");
}

#[test]
fn pointed_zero_version_database_is_rejected_without_initializing_it() {
    let root = tempfile::tempdir().unwrap();
    let active = host(root.path());
    let next = active
        .storage_parent()
        .unwrap()
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir(&next).unwrap();
    let database = next.join("library.db");
    drop(rusqlite::Connection::open(&database).unwrap());
    let pointer = Pointer {
        version: 1,
        generation: 2,
        location: Location::Storage {
            id: next.file_name().unwrap().to_str().unwrap().into(),
        },
    };
    write_pointer(root.path(), "active.json", &pointer).unwrap();
    drop(active);
    assert!(LibraryHost::open(root.path())
        .err()
        .unwrap()
        .contains("library_recovery_required"));
    let connection = rusqlite::Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn existing_flat_zero_version_database_is_never_initialized_even_with_orphan_assets() {
    for assets in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("library.db");
        drop(rusqlite::Connection::open(&database).unwrap());
        if assets {
            std::fs::create_dir(root.path().join("assets")).unwrap();
            std::fs::write(
                root.path().join("assets/orphan.png"),
                b"preserve user bytes",
            )
            .unwrap();
        }
        let before = std::fs::read(&database).unwrap();
        assert!(LibraryHost::open(root.path())
            .err()
            .unwrap()
            .contains("existing_library_unversioned"));
        assert_eq!(std::fs::read(&database).unwrap(), before);
        assert!(!root.path().join("cache").exists());
        if assets {
            assert_eq!(
                std::fs::read(root.path().join("assets/orphan.png")).unwrap(),
                b"preserve user bytes"
            );
        }
    }
}

#[test]
fn maintenance_barrier_waits_for_in_flight_import_write_and_rejects_late_old_generation() {
    let root = tempfile::tempdir().unwrap();
    let active = Arc::new(host(root.path()));
    let barrier = Arc::new(Barrier::new(2));
    let (entered_tx, entered_rx) = mpsc::channel();
    let worker = {
        let active = active.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            active.write(1, |s| {
                entered_tx.send(()).unwrap();
                barrier.wait();
                add_case(s, "in-flight");
                Ok(())
            })
        })
    };
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (maint_tx, maint_rx) = mpsc::channel();
    let maintenance = {
        let active = active.clone();
        std::thread::spawn(move || {
            let guard = active.begin_maintenance(1).unwrap();
            maint_tx.send(active.revision()).unwrap();
            drop(guard)
        })
    };
    assert!(maint_rx.recv_timeout(Duration::from_millis(50)).is_err());
    barrier.wait();
    worker.join().unwrap().unwrap();
    assert_eq!(maint_rx.recv_timeout(Duration::from_secs(3)).unwrap(), 1);
    maintenance.join().unwrap();
    assert_eq!(active.read(1, |s| Ok(title(s))).unwrap(), "in-flight");
}

// This worker is only started by the next test against its marked temporary root. It parks
// after real durable file operations; the parent kills its own process to test startup repair.
#[test]
fn switch_interruption_worker() {
    let Some(root) = std::env::var_os("PROMPT_SHELF_HOST_WORKER_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(root.join("host-test-owned").is_file());
    let active = host(&root);
    let revision = active.revision();
    let path = PathBuf::from(std::env::var_os("PROMPT_SHELF_HOST_WORKER_STORAGE").unwrap());
    let stage = std::env::var("PROMPT_SHELF_HOST_WORKER_STAGE").unwrap();
    let mut maintenance = active
        .begin_maintenance(context(&active).generation)
        .unwrap();
    maintenance
        .install_inner(&path, revision, |point| {
            if matches!(
                (point, stage.as_str()),
                (SwitchPoint::PreviousDurable, "previous") | (SwitchPoint::ActiveDurable, "active")
            ) {
                let mut ready = File::create(root.join("worker-ready")).unwrap();
                ready.write_all(b"durable boundary reached").unwrap();
                ready.sync_all().unwrap();
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        })
        .unwrap();
    panic!("worker did not reach requested boundary")
}

#[test]
fn killed_worker_at_real_pointer_boundaries_reopens_a_complete_old_or_new_library() {
    for stage in ["previous", "active"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("host-test-owned"), b"owned temp").unwrap();
        let active = host(root.path());
        active
            .write(1, |s| {
                add_case(s, "old");
                Ok(())
            })
            .unwrap();
        let next = prepare(&active, "new");
        drop(active);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage_host::tests::switch_interruption_worker",
                "--nocapture",
            ])
            .env("PROMPT_SHELF_HOST_WORKER_ROOT", root.path())
            .env("PROMPT_SHELF_HOST_WORKER_STORAGE", &next)
            .env("PROMPT_SHELF_HOST_WORKER_STAGE", stage)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !root.path().join("worker-ready").is_file() && Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let reached = root.path().join("worker-ready").is_file();
        let _ = child.kill();
        child.wait().unwrap();
        assert!(reached, "worker failed to reach {stage}");
        let reopened = host(root.path());
        let generation = context(&reopened).generation;
        assert_eq!(
            reopened.read(generation, |s| Ok(title(s))).unwrap(),
            if stage == "active" { "new" } else { "old" }
        );
        reopened.read(generation, |s| s.check()).unwrap();
        assert!(root.path().join("library.db").is_file());
        assert!(next.join("library.db").is_file());
        eprintln!(
            "HOST_PROCESS_KILL_VERIFIED: {stage}, generation={generation}, platform={}",
            std::env::consts::OS
        );
    }
}
