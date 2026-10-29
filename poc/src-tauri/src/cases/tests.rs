use super::*;
use crate::{assets::hash_file, paths::DataPaths};
fn store(path: &std::path::Path) -> Store {
    Store::open(DataPaths::new(path).unwrap()).unwrap()
}
fn document(store: &Store, title: &str) -> CaseDocument {
    let mut d = store.edit_document(None).unwrap();
    d.title = title.into();
    d.prompt = "完整第一行\n完整第二行".into();
    d
}
fn image(store: &Store, root: &std::path::Path, name: &str, color: u8) -> String {
    let file = root.join(name);
    image::RgbaImage::from_pixel(12, 8, image::Rgba([color, 80, 120, 96]))
        .save(&file)
        .unwrap();
    store.import_image(&file).unwrap().id
}
fn reference(asset: &str, name: &str) -> ImageReference {
    ImageReference {
        id: uuid(),
        asset_id: asset.into(),
        original_name: name.into(),
    }
}
#[test]
fn new_save_catalogs_cancel_and_reopen_are_transactional() {
    let root = tempfile::tempdir().unwrap();
    let first = store(root.path());
    let mut d = document(&first, "新案例");
    d.project = " Project ".into();
    d.tags = vec![
        " 标签 ".into(),
        "标签".into(),
        "Other".into(),
        "other".into(),
    ];
    d.source_url = "例子.测试/source".into();
    assert_eq!(first.counts().unwrap(), (0, 0));
    let sessions = EditSessions::default();
    let receipt = sessions.begin_at(d.clone(), sessions.epoch()).unwrap();
    sessions.cancel(&receipt.session_id).unwrap();
    assert!(first.library().unwrap().projects.is_empty());
    let saved = first.commit_case(&d).unwrap();
    assert_eq!(saved.id, d.id);
    assert_eq!(saved.edit_revision, 1);
    let mut second = document(&first, "复用分类");
    second.project = "project".into();
    second.tags = vec!["OTHER".into()];
    first.commit_case(&second).unwrap();
    let library = first.library().unwrap();
    assert_eq!(library.projects.len(), 1);
    assert_eq!(library.tags.len(), 2);
    assert!(library.tags.iter().all(|t| !t.is_quick));
    assert!(library
        .cases
        .iter()
        .all(|c| c.used == 0 && c.last_used_at.is_none()));
    drop(first);
    let reopened = store(root.path());
    let loaded = reopened.edit_document(Some(&d.id)).unwrap();
    assert_eq!(loaded.id, d.id);
    assert_eq!(loaded.tags, vec!["标签", "Other"]);
    assert_eq!(loaded.source_url, "https://xn--fsqu00a.xn--0zwm56d/source");
    assert_eq!(loaded.prompt, d.prompt);
}
#[test]
fn shared_images_reference_ids_and_reorder_preserve_other_case_and_originals() {
    let root = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let store = store(root.path());
    let shared = image(&store, files.path(), "共享.png", 10);
    let replacement = image(&store, files.path(), "替换.png", 20);
    let mut a = document(&store, "A");
    a.result = Some(reference(&shared, "效果"));
    a.references = vec![
        reference(&shared, "一"),
        reference(&replacement, "二"),
        reference(&shared, "三"),
    ];
    let mut b = document(&store, "B");
    b.references = vec![reference(&shared, "另案")];
    store.commit_case(&a).unwrap();
    store.commit_case(&b).unwrap();
    let originals = store
        .library()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| {
            let hash = hash_file(std::path::Path::new(&asset.original)).unwrap();
            (asset.original, hash)
        })
        .collect::<Vec<_>>();
    let before_b = serde_json::to_value(store.edit_document(Some(&b.id)).unwrap()).unwrap();
    let mut edit = store.edit_document(Some(&a.id)).unwrap();
    let stable = edit.references[2].id.clone();
    edit.references.swap(0, 2);
    edit.references[0].asset_id = replacement;
    edit.result = None;
    store.commit_case(&edit).unwrap();
    let loaded = store.edit_document(Some(&a.id)).unwrap();
    assert_eq!(loaded.references[0].id, stable);
    assert_eq!(loaded.references.len(), 3);
    assert!(loaded.result.is_none());
    assert_eq!(
        serde_json::to_value(store.edit_document(Some(&b.id)).unwrap()).unwrap(),
        before_b
    );
    for (path, hash) in originals {
        assert_eq!(hash_file(std::path::Path::new(&path)).unwrap(), hash);
    }
    assert_eq!(
        std::fs::read_dir(root.path().join("assets"))
            .unwrap()
            .count(),
        2
    );
}
#[test]
fn sql_failure_after_field_catalog_and_relation_changes_rolls_everything_back() {
    let root = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let store = store(root.path());
    let asset = image(&store, files.path(), "x.png", 10);
    let mut original = document(&store, "旧正式");
    original.tags = vec!["旧标签".into()];
    original.references = vec![reference(&asset, "old")];
    store.commit_case(&original).unwrap();
    store.connection.lock().unwrap().execute_batch("CREATE TRIGGER injected_save_failure BEFORE INSERT ON case_assets WHEN NEW.original_name='fail' BEGIN SELECT RAISE(ABORT,'injected_save_failure'); END;").unwrap();
    let before = store.edit_document(Some(&original.id)).unwrap();
    let mut edit = before.clone();
    edit.title = "新标题".into();
    edit.project = "失败项目".into();
    edit.tags = vec!["失败标签".into()];
    edit.references[0].original_name = "fail".into();
    assert!(store.commit_case(&edit).is_err());
    assert_eq!(
        serde_json::to_value(store.edit_document(Some(&original.id)).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    let library = store.library().unwrap();
    assert!(library.projects.is_empty());
    assert_eq!(library.tags.len(), 1);
    let mut new = document(&store, "失败新建");
    new.project = "失败项目".into();
    new.references = vec![reference(&asset, "fail")];
    assert!(store.commit_case(&new).is_err());
    assert_eq!(store.counts().unwrap().0, 1);
    assert!(store.library().unwrap().projects.is_empty());
}
#[test]
fn stale_save_and_stale_quick_index_are_rejected_without_changing_usage() {
    let root = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let store = store(root.path());
    let one = image(&store, files.path(), "one.png", 1);
    let two = image(&store, files.path(), "two.png", 2);
    let mut d = document(&store, "版本");
    d.references = vec![reference(&one, "one"), reference(&two, "two")];
    store.commit_case(&d).unwrap();
    store
        .connection
        .lock()
        .unwrap()
        .execute(
            "UPDATE cases SET use_count=7,last_used_at=42,favorite=1 WHERE id=?1",
            [&d.id],
        )
        .unwrap();
    let stale = store.edit_document(Some(&d.id)).unwrap();
    let mut fresh = stale.clone();
    fresh.references.reverse();
    store.commit_case(&fresh).unwrap();
    assert!(store
        .case_snapshot_at_revision(&d.id, 1)
        .err()
        .unwrap()
        .contains("case_changed"));
    assert!(store
        .commit_case(&stale)
        .unwrap_err()
        .contains("revision_conflict"));
    let snapshot = store.case_snapshot_at_revision(&d.id, 2).unwrap();
    assert_eq!(snapshot.cases[0].refs, vec![two, one]);
    assert_eq!(snapshot.cases[0].used, 7);
    assert_eq!(snapshot.cases[0].last_used_at, Some(42));
    assert!(snapshot.cases[0].favorite);
}
#[test]
fn missing_original_or_unknown_asset_never_saves_formal_case() {
    let root = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let store = store(root.path());
    let id = image(&store, files.path(), "x.png", 1);
    let mut d = document(&store, "缺失");
    d.project = "不应登记".into();
    d.references = vec![reference(&id, "x")];
    let relative:String=store.connection.lock().unwrap().query_row("SELECT relative_path FROM assets WHERE id=?1",[&id],|r|r.get(0)).unwrap();
    let original = store.paths().resolve(&relative,"assets").unwrap();
    std::fs::remove_file(original).unwrap();
    assert!(store.commit_case(&d).is_err());
    assert_eq!(store.counts().unwrap().0, 0);
    assert!(
        store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
            .unwrap()
            == 0
    );
}
#[test]
fn memory_sessions_do_not_revive_after_cancel_or_close_and_errors_keep_editing() {
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path());
    let sessions = EditSessions::default();
    let receipt = sessions
        .begin_at(store.edit_document(None).unwrap(), 0)
        .unwrap();
    assert!(sessions
        .save(&store, &receipt.session_id, &receipt.document)
        .is_err());
    sessions.check(&receipt.session_id).unwrap();
    sessions.cancel(&receipt.session_id).unwrap();
    assert!(sessions.check(&receipt.session_id).is_err());
    assert_eq!(store.counts().unwrap().0, 0);
    let old_epoch = sessions.epoch();
    sessions.clear();
    assert!(sessions
        .begin_at(store.edit_document(None).unwrap(), old_epoch)
        .is_err());
    let next = sessions
        .begin_at(document(&store, "后一个"), sessions.epoch())
        .unwrap();
    assert!(sessions.cancel(&receipt.session_id).is_err());
    sessions.check(&next.session_id).unwrap();
    sessions
        .save(&store, &next.session_id, &next.document)
        .unwrap();
    assert!(sessions.check(&next.session_id).is_err());
}
#[test]
fn source_normalization_uses_http_https_and_idn() {
    assert_eq!(
        normalize_source(" HTTPS://Example.com/a ").unwrap(),
        "https://example.com/a"
    );
    assert_eq!(
        normalize_source("example.com:8080/a").unwrap(),
        "https://example.com:8080/a"
    );
    assert_eq!(normalize_source("").unwrap(), "");
    for invalid in [
        "javascript:alert(1)",
        "javascript:123",
        "data:image/png,abc",
        "file:///C:/x",
        "mailto:a@b.com",
        "https://exam\nple.com/",
    ] {
        assert!(normalize_source(invalid).is_err(), "{invalid}");
    }
}
