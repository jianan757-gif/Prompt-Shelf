use super::*;
use crate::paths::DataPaths;
fn store(root: &std::path::Path) -> Store {
    Store::open(DataPaths::new(root).unwrap()).unwrap()
}
fn formal(s: &Store) -> CaseDocument {
    let mut d = s.edit_document(None).unwrap();
    d.title = "正式 A".into();
    d.prompt = "正式正文".into();
    d.project = "已有项目".into();
    d.tags = vec!["已有标签".into()];
    s.commit_case(&d).unwrap();
    s.edit_document(Some(&d.id)).unwrap()
}
#[test]
fn incomplete_draft_catalog_inputs_and_deleted_bound_categories() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let original = formal(&s);
    let sessions = EditSessions::default();
    let receipt = sessions
        .begin_persistent(&s, Some(&original.id), None, false, sessions.epoch())
        .unwrap();
    let draft = receipt.draft_id.unwrap();
    let mut d = receipt.document;
    d.title.clear();
    d.prompt.clear();
    d.source_url = "unfinished : invalid".into();
    d.tags.push("新标签".into());
    sessions.stash(&s, &receipt.session_id, &d, 2).unwrap();
    assert!(sessions
        .stash(&s, &receipt.session_id, &d, 1)
        .unwrap_err()
        .contains("stale"));
    let library = s.library().unwrap();
    assert_eq!(library.projects.len(), 1);
    assert_eq!(library.tags.len(), 1);
    assert_eq!(library.cases[0].title, "正式 A");
    {
        let c = s.connection.lock().unwrap();
        c.execute("DELETE FROM projects", []).unwrap();
        c.execute("DELETE FROM tags", []).unwrap();
    }
    let loaded = s.draft_document(&draft).unwrap().0;
    assert!(loaded.project.is_empty());
    assert_eq!(loaded.tags, vec!["新标签"]);
    assert!(loaded.title.is_empty());
    assert_eq!(loaded.source_url, d.source_url);
    d.project = "尚未登记项目".into();
    d.tags = vec!["未登记标签".into()];
    sessions.stash(&s, &receipt.session_id, &d, 3).unwrap();
    assert!(s.library().unwrap().projects.is_empty());
    assert_eq!(s.draft_document(&draft).unwrap().0.project, d.project);
    sessions.discard(&s, &receipt.session_id).unwrap();
    assert!(sessions.stash(&s, &receipt.session_id, &d, 4).is_err());
    assert!(s.update_draft(&draft, &d, 5).is_err());
    assert!(s.draft_summaries().unwrap().is_empty());
}
#[test]
fn save_and_discard_failures_preserve_formal_draft_session_and_close_guard() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let original = formal(&s);
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, Some(&original.id), None, false, sessions.epoch())
        .unwrap();
    let draft = r.draft_id.clone().unwrap();
    let mut d = r.document;
    d.title = "编辑 B".into();
    d.project = "新项目".into();
    d.tags.push("新标签".into());
    sessions.stash(&s, &r.session_id, &d, 1).unwrap();
    s.connection.lock().unwrap().execute_batch("CREATE TRIGGER fail_consume BEFORE DELETE ON drafts BEGIN SELECT RAISE(ABORT,'injected_consume'); END;").unwrap();
    assert!(sessions
        .save(&s, &r.session_id, &d)
        .unwrap_err()
        .contains("injected_consume"));
    assert_eq!(s.edit_document(Some(&d.id)).unwrap().title, original.title);
    assert_eq!(s.library().unwrap().projects.len(), 1);
    assert_eq!(s.library().unwrap().tags.len(), 1);
    assert_eq!(s.draft_document(&draft).unwrap().0.title, d.title);
    assert!(sessions.prepare_close().is_err());
    assert!(sessions.discard(&s, &r.session_id).is_err());
    assert!(sessions.active());
    s.connection
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_consume")
        .unwrap();
    sessions.save(&s, &r.session_id, &d).unwrap();
    assert!(s.draft_document(&draft).is_err());
    assert!(sessions.stash(&s, &r.session_id, &d, 2).is_err());
    assert!(sessions.prepare_close().is_ok());
    assert_eq!(s.edit_document(Some(&d.id)).unwrap().title, "编辑 B");
}
#[test]
fn draft_write_rollback_session_epoch_and_close_do_not_resurrect() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let sessions = EditSessions::default();
    let epoch = sessions.epoch();
    let r = sessions
        .begin_persistent(&s, None, None, false, epoch)
        .unwrap();
    let draft = r.draft_id.clone().unwrap();
    let mut d = r.document;
    d.title = "第一份".into();
    sessions.stash(&s, &r.session_id, &d, 1).unwrap();
    s.connection.lock().unwrap().execute_batch("CREATE TRIGGER fail_pending BEFORE INSERT ON draft_pending_tags BEGIN SELECT RAISE(ABORT,'injected_draft'); END;").unwrap();
    d.title = "失败修改".into();
    d.tags = vec!["新标签".into()];
    assert!(sessions.stash(&s, &r.session_id, &d, 2).is_err());
    let loaded = s.draft_document(&draft).unwrap();
    assert_eq!(loaded.0.title, "第一份");
    assert_eq!(loaded.1, 1);
    sessions.clear();
    assert!(sessions.stash(&s, &r.session_id, &d, 3).is_err());
    assert!(sessions
        .begin_persistent(&s, None, None, false, epoch)
        .is_err());
    assert_eq!(s.draft_document(&draft).unwrap().0.title, "第一份");
    assert!(sessions.prepare_close().is_ok());
}
#[test]
fn conflict_deleted_target_and_explicit_as_new_never_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let original = formal(&s);
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, Some(&original.id), None, false, sessions.epoch())
        .unwrap();
    let draft = r.draft_id.clone().unwrap();
    let mut d = r.document;
    d.title = "旧基线的草稿".into();
    sessions.stash(&s, &r.session_id, &d, 1).unwrap();
    sessions.clear();
    let mut changed = original.clone();
    changed.title = "新正式".into();
    s.commit_case(&changed).unwrap();
    assert!(s.draft_summaries().unwrap()[0].conflict);
    let recovered = sessions
        .begin_persistent(&s, None, Some(&draft), false, sessions.epoch())
        .unwrap();
    assert!(sessions
        .save(&s, &recovered.session_id, &recovered.document)
        .unwrap_err()
        .contains("revision_conflict"));
    sessions.clear();
    s.connection
        .lock()
        .unwrap()
        .execute("UPDATE cases SET deleted_at=1 WHERE id=?1", [&d.id])
        .unwrap();
    assert!(sessions
        .begin_persistent(&s, None, Some(&draft), false, sessions.epoch())
        .is_err());
    let new = sessions
        .begin_persistent(&s, None, Some(&draft), true, sessions.epoch())
        .unwrap();
    assert_ne!(new.document.id, d.id);
    assert_eq!(new.document.base_revision, 0);
    assert!(s.draft_document(&draft).is_err());
    assert_eq!(s.counts().unwrap().0, 0);
    sessions.save(&s, &new.session_id, &new.document).unwrap();
    assert_eq!(s.counts().unwrap().0, 1);
}
#[test]
fn migration_v1_to_v2_and_failed_upgrade_are_atomic() {
    let root = tempfile::tempdir().unwrap();
    let paths = DataPaths::new(root.path()).unwrap();
    let c = Connection::open(paths.database().unwrap()).unwrap();
    c.execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    c.execute_batch("PRAGMA user_version=1;INSERT INTO projects VALUES('p','保留','保留',0,1);CREATE TABLE draft_pending_tags(block INTEGER);").unwrap();
    drop(c);
    assert!(Store::open(paths.clone()).is_err());
    let c = Connection::open(paths.database().unwrap()).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT name FROM projects", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "保留"
    );
    assert!(c.prepare("SELECT dirty FROM drafts").is_err());
    c.execute("DROP TABLE draft_pending_tags", []).unwrap();
    drop(c);
    let s = Store::open(paths).unwrap();
    assert_eq!(s.check().unwrap()["schemaVersion"], 2);
    assert_eq!(s.library().unwrap().projects[0].name, "保留");
}

#[test]
fn new_case_failed_commit_and_draft_image_reorder_keep_shared_original() {
    let root = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let source = files.path().join("shared.png");
    image::RgbaImage::from_pixel(6, 4, image::Rgba([10, 20, 30, 90]))
        .save(&source)
        .unwrap();
    let asset = s.import_image(&source).unwrap();
    let mut a = formal(&s);
    a.references = vec![ImageReference {
        id: id(),
        asset_id: asset.id.clone(),
        original_name: "formal.png".into(),
    }];
    s.commit_case(&a).unwrap();
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, None, None, false, sessions.epoch())
        .unwrap();
    let draft = r.draft_id.clone().unwrap();
    let mut d = r.document;
    d.title = "新案例".into();
    d.prompt = "正文".into();
    d.project = "待登记项目".into();
    d.references = vec![
        ImageReference {
            id: id(),
            asset_id: asset.id.clone(),
            original_name: "first.png".into(),
        },
        ImageReference {
            id: id(),
            asset_id: asset.id.clone(),
            original_name: "second.png".into(),
        },
    ];
    sessions.stash(&s, &r.session_id, &d, 1).unwrap();
    d.references.swap(0, 1);
    sessions.stash(&s, &r.session_id, &d, 2).unwrap();
    assert_eq!(
        s.draft_document(&draft).unwrap().0.references[0].id,
        d.references[0].id
    );
    s.connection.lock().unwrap().execute_batch("CREATE TRIGGER fail_new_consume BEFORE DELETE ON drafts BEGIN SELECT RAISE(ABORT,'new_consume_failure'); END;").unwrap();
    assert!(sessions.save(&s, &r.session_id, &d).is_err());
    assert_eq!(s.counts().unwrap().0, 1);
    assert_eq!(s.library().unwrap().projects.len(), 1);
    assert_eq!(s.draft_document(&draft).unwrap().0.references.len(), 2);
    s.connection
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_new_consume")
        .unwrap();
    sessions.discard(&s, &r.session_id).unwrap();
    assert!(s.draft_document(&draft).is_err());
    assert_eq!(
        s.edit_document(Some(&a.id)).unwrap().references[0].id,
        a.references[0].id
    );
    assert_eq!(
        crate::assets::hash_file(std::path::Path::new(&asset.original)).unwrap(),
        asset.sha256
    );
}

#[test]
fn delayed_worker_after_discard_save_and_window_close_cannot_write() {
    use std::sync::{mpsc, Arc};
    for ending in ["discard", "save", "destroy"] {
        let root = tempfile::tempdir().unwrap();
        let s = Arc::new(store(root.path()));
        let sessions = Arc::new(EditSessions::default());
        let r = sessions
            .begin_persistent(&s, None, None, false, sessions.epoch())
            .unwrap();
        let mut d = r.document;
        d.title = "草稿".into();
        d.prompt = "内容".into();
        sessions.stash(&s, &r.session_id, &d, 1).unwrap();
        let draft = r.draft_id.unwrap();
        let (release, wait) = mpsc::channel();
        let worker_sessions = sessions.clone();
        let worker_store = s.clone();
        let token = r.session_id.clone();
        let worker_document = d.clone();
        let worker = std::thread::spawn(move || {
            wait.recv().unwrap();
            worker_sessions.stash(&worker_store, &token, &worker_document, 99)
        });
        match ending {
            "discard" => sessions.discard(&s, &r.session_id).unwrap(),
            "save" => {
                sessions.save(&s, &r.session_id, &d).unwrap();
            }
            _ => sessions.clear(),
        }
        release.send(()).unwrap();
        assert!(worker.join().unwrap().is_err());
        assert!(!sessions.active());
        if ending == "destroy" {
            assert_eq!(s.draft_document(&draft).unwrap().1, 1)
        } else {
            assert!(s.draft_document(&draft).is_err())
        }
    }
}
