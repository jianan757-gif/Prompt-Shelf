use super::*;
use crate::{
    cases::{CaseDocument, ImageReference},
    paths::DataPaths,
};
use std::path::Path;

fn open(root: &Path) -> Store {
    Store::open(DataPaths::new(root).unwrap()).unwrap()
}
fn image(store: &Store, source: &Path, color: u8) -> data::Asset {
    image::RgbaImage::from_pixel(8, 6, image::Rgba([color, 90, 150, 255]))
        .save(source)
        .unwrap();
    store.import_image(source).unwrap()
}
fn reference(asset: &data::Asset) -> ImageReference {
    ImageReference {
        id: uuid::Uuid::new_v4().to_string(),
        asset_id: asset.id.clone(),
        original_name: "测试.png".into(),
    }
}
fn case(store: &Store, title: &str, images: &[&data::Asset]) -> String {
    let d = CaseDocument {
        id: uuid::Uuid::new_v4().to_string(),
        base_revision: 0,
        title: title.into(),
        prompt: "完整正文第一行\n第二行\n第三行".into(),
        project: "原项目".into(),
        tags: vec!["标签".into()],
        source_url: "https://example.com/source".into(),
        result: images.first().map(|asset| reference(asset)),
        references: images.iter().map(|asset| reference(asset)).collect(),
    };
    store.commit_case(&d).unwrap().id
}

#[test]
fn soft_delete_and_restore_reuse_persisted_content_and_relationships() {
    let root = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store = open(root.path());
    let asset = image(&store, &source.path().join("a.png"), 20);
    let id = case(&store, "完整案例", &[&asset, &asset]);
    store.set_favorite(&id, true).unwrap();
    store.record_use(&id).unwrap();
    let before = store.library_index().unwrap().cases.remove(0);
    store.soft_delete_case(&id).unwrap();
    let index = store.library_index().unwrap();
    assert!(index.cases.is_empty());
    assert_eq!(index.trash_count, 1);
    assert!(store.edit_document(Some(&id)).is_err());
    assert!(store.assets_for_cases(&[id.clone()]).unwrap().is_empty());
    let trash = store.trash_index().unwrap().remove(0);
    assert_eq!(trash.case.id, id);
    assert_eq!(trash.deleted_project_name, "原项目");
    assert_eq!(
        trash.shared_image_count, 0,
        "same-case duplicate references are not sharing"
    );
    assert_eq!(
        serde_json::to_value(&trash.case).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(store.trash_assets(&[id.clone()]).unwrap().len(), 1);
    assert!(store.soft_delete_case(&id).is_err());
    store.restore_case(&id).unwrap();
    assert!(store.trash_index().unwrap().is_empty());
    assert_eq!(store.library_index().unwrap().trash_count, 0);
    assert_eq!(
        serde_json::to_value(&store.library_index().unwrap().cases[0]).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert!(store.restore_case(&id).is_err());
    assert!(Path::new(&asset.original).exists());
}

#[test]
fn shared_count_uses_other_case_entities_including_trash_but_not_drafts() {
    let root = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store = open(root.path());
    let shared = image(&store, &source.path().join("shared.png"), 30);
    let draft_only = image(&store, &source.path().join("draft.png"), 40);
    let a = case(&store, "案例A", &[&shared, &shared, &draft_only]);
    let b = case(&store, "案例B", &[&shared]);
    {
        let c = store.connection.lock().unwrap();
        c.execute(
            "INSERT INTO drafts(id,updated_at,dirty) VALUES('independent',1,1)",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO draft_assets(id,draft_id,asset_id,role,position,original_name) VALUES('draft-image','independent',?1,'reference',0,'draft.png')", [&draft_only.id]).unwrap();
    }
    store.soft_delete_case(&a).unwrap();
    assert_eq!(store.trash_index().unwrap()[0].shared_image_count, 1);
    store.soft_delete_case(&b).unwrap();
    assert!(store
        .trash_index()
        .unwrap()
        .iter()
        .all(|row| row.shared_image_count == 1));
    store.permanently_delete_case(&b).unwrap();
    assert_eq!(store.trash_index().unwrap()[0].shared_image_count, 0);
    store.permanently_delete_case(&a).unwrap();
    assert!(store.trash_index().unwrap().is_empty());
    let c = store.connection.lock().unwrap();
    assert_eq!(
        c.query_row(
            "SELECT gc_state FROM assets WHERE id=?1",
            [&shared.id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "pending"
    );
    assert_eq!(
        c.query_row(
            "SELECT gc_state FROM assets WHERE id=?1",
            [&draft_only.id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "live"
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM draft_assets", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        Path::new(&shared.original).exists(),
        "permanent delete must never unlink files"
    );
    assert!(Path::new(&draft_only.original).exists());
}

#[test]
fn deleted_project_name_survives_project_removal_and_reopen_then_restores_independent() {
    let root = tempfile::tempdir().unwrap();
    let store = open(root.path());
    let id = case(&store, "项目案例", &[]);
    let project = store.library_index().unwrap().projects[0].id.clone();
    store.soft_delete_case(&id).unwrap();
    store.catalog_delete("projects", &[project]).unwrap();
    drop(store);
    let store = open(root.path());
    let row = store.trash_index().unwrap().remove(0);
    assert_eq!(row.deleted_project_name, "原项目");
    assert!(row.case.project_id.is_none());
    assert!(row.case.project.is_empty());
    store.restore_case(&id).unwrap();
    let restored = store.library_index().unwrap().cases.remove(0);
    assert!(restored.project_id.is_none());
    assert!(restored.project.is_empty());
    assert_eq!(restored.tags, ["标签"]);
}

#[test]
fn all_related_drafts_require_explicit_resolution_before_delete() {
    let root = tempfile::tempdir().unwrap();
    let store = open(root.path());
    let id = case(&store, "草稿案例", &[]);
    for dirty in [0, 1] {
        store.connection.lock().unwrap().execute("INSERT INTO drafts(id,case_id,base_revision,updated_at,dirty) VALUES('related',?1,1,1,?2)", params![id, dirty]).unwrap();
        assert_eq!(
            store.case_delete_draft(&id).unwrap().as_deref(),
            Some("related")
        );
        assert_eq!(
            store.soft_delete_case(&id).unwrap_err(),
            "case_has_unfinished_draft_resolve_first"
        );
        assert_eq!(store.library_index().unwrap().cases.len(), 1);
        store.discard_draft("related").unwrap();
        assert!(store.case_delete_draft(&id).unwrap().is_none());
    }
    assert_eq!(
        store.permanently_delete_case(&id).unwrap_err(),
        "case_not_in_trash"
    );
    store.soft_delete_case(&id).unwrap();
    store.connection.lock().unwrap().execute("INSERT INTO drafts(id,case_id,base_revision,updated_at,dirty) VALUES('related',?1,1,1,1)", [&id]).unwrap();
    assert_eq!(
        store.trash_index().unwrap()[0]
            .unfinished_draft_id
            .as_deref(),
        Some("related")
    );
    assert_eq!(
        store.permanently_delete_case(&id).unwrap_err(),
        "case_has_unfinished_draft_resolve_first"
    );
    assert_eq!(store.trash_index().unwrap().len(), 1);
    store.restore_case(&id).unwrap();
    assert_eq!(
        store.draft_summaries().unwrap().len(),
        1,
        "restore preserves unfinished work"
    );
    store.discard_draft("related").unwrap();
    store.soft_delete_case(&id).unwrap();
    store.permanently_delete_case(&id).unwrap();
    assert!(store.library_index().unwrap().cases.is_empty());
    assert!(store.trash_index().unwrap().is_empty());
}

#[test]
fn permanent_delete_failure_rolls_back_case_and_reference_changes() {
    let root = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store = open(root.path());
    let asset = image(&store, &source.path().join("a.png"), 50);
    let id = case(&store, "事务案例", &[&asset]);
    store.soft_delete_case(&id).unwrap();
    store.connection.lock().unwrap().execute_batch("CREATE TRIGGER reject_pending BEFORE UPDATE OF gc_state ON assets WHEN NEW.gc_state='pending' BEGIN SELECT RAISE(ABORT,'injected_gc_mark_failure'); END;").unwrap();
    assert!(store
        .permanently_delete_case(&id)
        .unwrap_err()
        .contains("injected_gc_mark_failure"));
    assert_eq!(store.trash_index().unwrap().len(), 1);
    assert_eq!(store.trash_assets(&[id.clone()]).unwrap().len(), 1);
    store
        .connection
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_pending")
        .unwrap();
    store.permanently_delete_case(&id).unwrap();
    assert!(store.trash_index().unwrap().is_empty());
    assert!(Path::new(&asset.original).exists());
    assert_eq!(
        store
            .connection
            .lock()
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
