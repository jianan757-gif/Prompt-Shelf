use super::*;
use crate::{
    cases::{CaseDocument, EditSessions, ImageReference},
    paths::DataPaths,
};
fn store(root: &std::path::Path) -> Store {
    Store::open(DataPaths::new(root).unwrap()).unwrap()
}
fn formal(s: &Store) -> CaseDocument {
    let mut d = s.edit_document(None).unwrap();
    d.title = "正式案例".into();
    d.prompt = "完整提示词".into();
    d.project = "项目".into();
    d.tags = vec!["标签".into(), "第二标签".into()];
    s.commit_case(&d).unwrap();
    s.edit_document(Some(&d.id)).unwrap()
}

#[test]
fn rename_and_delete_preserve_formal_and_draft_id_relations_without_resurrection() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let d = formal(&s);
    let before = s.library_index().unwrap();
    let project = before.cases[0].project_id.clone().unwrap();
    let tag = before.cases[0].tag_ids[0].clone();
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, Some(&d.id), None, false, sessions.epoch())
        .unwrap();
    let draft = r.draft_id.clone().unwrap();
    let mut edited = r.document;
    edited.title = "草稿".into();
    sessions.stash(&s, &r.session_id, &edited, 1).unwrap();
    sessions.clear();
    s.catalog_rename("projects", &project, " 新项目 ").unwrap();
    s.catalog_rename("tags", &tag, "新标签").unwrap();
    let renamed = s.library_index().unwrap();
    assert_eq!(
        renamed.cases[0].project_id.as_deref(),
        Some(project.as_str())
    );
    assert_eq!(renamed.cases[0].tag_ids[0], tag);
    assert_eq!(renamed.cases[0].edit_revision, 1);
    let restored = s.draft_document(&draft).unwrap().0;
    assert_eq!(restored.project, "新项目");
    assert_eq!(restored.tags[0], "新标签");
    s.catalog_delete("projects", &[project]).unwrap();
    s.catalog_delete("tags", &[tag]).unwrap();
    let normalized = s.draft_document(&draft).unwrap().0;
    assert!(normalized.project.is_empty());
    assert_eq!(normalized.tags, vec!["第二标签"]);
    let r = sessions
        .begin_persistent(&s, None, Some(&draft), false, sessions.epoch())
        .unwrap();
    sessions.save(&s, &r.session_id, &r.document).unwrap();
    let after = s.library_index().unwrap();
    assert!(after.projects.is_empty());
    assert_eq!(after.tags.len(), 1);
    assert_eq!(after.cases.len(), 1);
    assert_eq!(after.cases[0].title, "草稿");
    assert!(after.cases[0].project_id.is_none());
}
#[test]
fn previously_unresolved_names_follow_registration_rename_and_delete() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, None, None, false, sessions.epoch())
        .unwrap();
    let draft = r.draft_id.clone().unwrap();
    let mut d = r.document;
    d.project = " PendingProject ".into();
    d.tags = vec!["pendingTag".into(), "独立待登记".into()];
    sessions.stash(&s, &r.session_id, &d, 1).unwrap();
    sessions.clear();
    let project = s.catalog_create("projects", "PendingProject").unwrap();
    let tag = s.catalog_create("tags", "PendingTag").unwrap();
    s.catalog_rename("projects", &project, "已改项目").unwrap();
    s.catalog_rename("tags", &tag, "已改标签").unwrap();
    let loaded = s.draft_document(&draft).unwrap().0;
    assert_eq!(loaded.project, "已改项目");
    assert_eq!(loaded.tags, vec!["已改标签", "独立待登记"]);
    s.catalog_delete("projects", &[project]).unwrap();
    s.catalog_delete("tags", &[tag]).unwrap();
    let loaded = s.draft_document(&draft).unwrap().0;
    assert!(loaded.project.is_empty());
    assert_eq!(loaded.tags, vec!["独立待登记"]);
    let id = s.catalog_create("tags", "独立待登记").unwrap();
    s.catalog_delete("tags", &[id]).unwrap();
    assert!(s.draft_document(&draft).unwrap().0.tags.is_empty());
}
#[test]
fn bulk_validates_all_ids_and_rolls_back_fk_changes_on_actual_sql_failure() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let d = formal(&s);
    let library = s.library_index().unwrap();
    let ids = library.cases[0].tag_ids.clone();
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, Some(&d.id), None, false, sessions.epoch())
        .unwrap();
    let draft = r.draft_id.unwrap();
    let mut edited = r.document;
    edited.title = "草稿".into();
    sessions.stash(&s, &r.session_id, &edited, 1).unwrap();
    sessions.clear();
    assert!(s
        .catalog_delete("tags", &[ids[0].clone(), "missing".into()])
        .is_err());
    assert_eq!(s.library_index().unwrap().tags.len(), 2);
    s.connection.lock().unwrap().execute_batch(&format!("CREATE TRIGGER fail_delete BEFORE DELETE ON tags WHEN old.id='{}' BEGIN SELECT RAISE(ABORT,'injected_catalog_delete'); END;",ids[1])).unwrap();
    assert!(s
        .catalog_delete("tags", &ids)
        .unwrap_err()
        .contains("injected_catalog_delete"));
    assert_eq!(s.library_index().unwrap().cases[0].tag_ids, ids);
    assert_eq!(
        s.draft_document(&draft).unwrap().0.tags,
        vec!["标签", "第二标签"]
    );
    assert_eq!(s.counts().unwrap().0, 1);
    assert!(s.catalog_create("tags", " 标签 ").is_err());
    assert!(s.catalog_create("projects", "\n").is_err());
    assert!(s
        .catalog_rename("projects", &library.projects[0].id, "bad\nname")
        .is_err());
    assert!(s.catalog_delete("wrong", &ids).is_err());
}
#[test]
fn quick_order_is_one_persisted_source_and_rejects_stale_or_duplicate_sets() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let ids = (0..3)
        .map(|n| s.catalog_create("tags", &format!("tag{n}")).unwrap())
        .collect::<Vec<_>>();
    for id in &ids {
        s.set_quick_tag(id, true).unwrap()
    }
    s.reorder_quick_tags(&[ids[2].clone(), ids[0].clone(), ids[1].clone()])
        .unwrap();
    assert!(s.reorder_quick_tags(&ids[..2]).is_err());
    assert!(s
        .reorder_quick_tags(&[ids[0].clone(), ids[0].clone(), ids[1].clone()])
        .is_err());
    s.set_quick_tag(&ids[2], false).unwrap();
    let order = s
        .library_index()
        .unwrap()
        .tags
        .into_iter()
        .filter(|t| t.is_quick)
        .map(|t| (t.id, t.quick_order))
        .collect::<Vec<_>>();
    assert_eq!(order, vec![(ids[0].clone(), 0), (ids[1].clone(), 1)]);
    drop(s);
    let s = store(root.path());
    assert_eq!(
        s.library_index()
            .unwrap()
            .tags
            .into_iter()
            .filter(|t| t.is_quick)
            .map(|t| (t.id, t.quick_order))
            .collect::<Vec<_>>(),
        order
    );
}
#[test]
fn favorite_is_immediate_and_draft_save_does_not_overwrite_it() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let d = formal(&s);
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, Some(&d.id), None, false, sessions.epoch())
        .unwrap();
    let mut edit = r.document;
    edit.title = "改正文".into();
    sessions.stash(&s, &r.session_id, &edit, 1).unwrap();
    s.set_favorite(&d.id, true).unwrap();
    let case = &s.library_index().unwrap().cases[0];
    assert!(case.favorite);
    assert_eq!(case.edit_revision, 1);
    assert_eq!(case.used, 0);
    assert!(case.last_used_at.is_none());
    sessions.save(&s, &r.session_id, &edit).unwrap();
    assert!(s.library_index().unwrap().cases[0].favorite);
    drop(s);
    let s = store(root.path());
    assert!(s.library_index().unwrap().cases[0].favorite);
    assert!(s.set_favorite("missing", true).is_err());
}
#[test]
fn preferences_and_dictionary_validate_atomically_and_reopen_including_empty_dictionary() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    assert_eq!(s.ui_preferences().unwrap(), default_preferences());
    assert_eq!(s.dictionary().unwrap(), default_dictionary());
    s.set_ui_preferences(json!({"libraryLayout":"masonry","librarySort":"new","quickIntervalMs":900,"railWidth":300})).unwrap();
    let before = s.ui_preferences().unwrap();
    assert!(s
        .set_ui_preferences(json!({"libraryLayout":"regular","quickIntervalMs":2}))
        .is_err());
    assert_eq!(s.ui_preferences().unwrap(), before);
    assert!(s
        .set_ui_preferences(json!({"machinePath":"C:/private"}))
        .is_err());
    s.save_dictionary(vec![
        vec![" A ".into(), "a".into(), "B".into()],
        vec!["中".into(), "文".into()],
    ])
    .unwrap();
    assert_eq!(
        s.dictionary().unwrap(),
        vec![vec!["A", "B"], vec!["中", "文"]]
    );
    let before = s.dictionary().unwrap();
    assert!(s
        .save_dictionary(vec![
            vec!["a".into(), "b".into()],
            vec!["A".into(), "other".into()]
        ])
        .is_err());
    assert_eq!(s.dictionary().unwrap(), before);
    s.connection.lock().unwrap().execute_batch("CREATE TRIGGER fail_dictionary BEFORE INSERT ON synonym_groups BEGIN SELECT RAISE(ABORT,'injected_dictionary'); END;").unwrap();
    assert!(s
        .save_dictionary(vec![vec!["one".into(), "two".into()]])
        .is_err());
    assert_eq!(s.dictionary().unwrap(), before);
    s.connection
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_dictionary")
        .unwrap();
    drop(s);
    let s = store(root.path());
    assert_eq!(
        s.ui_preferences().unwrap(),
        json!({"libraryLayout":"masonry","librarySort":"new","catalogColumns":"single","tagSort":"time","projectSort":"time","railWidth":300,"secondaryWidth":320,"expandedCatalog":null,"quickIntervalMs":900})
    );
    assert_eq!(s.dictionary().unwrap(), before);
    s.save_dictionary(vec![]).unwrap();
    drop(s);
    assert!(store(root.path()).dictionary().unwrap().is_empty());
}
#[test]
fn index_never_decodes_images_and_asset_batches_only_include_requested_live_cases() {
    let root = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let file = files.path().join("source.png");
    image::RgbaImage::from_pixel(12, 8, image::Rgba([10, 30, 50, 120]))
        .save(&file)
        .unwrap();
    let asset = s.import_image(&file).unwrap();
    let mut d = formal(&s);
    d.references = vec![ImageReference {
        id: id(),
        asset_id: asset.id.clone(),
        original_name: "source.png".into(),
    }];
    s.commit_case(&d).unwrap();
    let thumbnail = std::path::PathBuf::from(&asset.thumbnail);
    std::fs::remove_file(&thumbnail).unwrap();
    assert!(s.library_index().unwrap().assets.is_empty());
    assert!(!thumbnail.exists());
    assert!(s.assets_for_cases(&["missing".into()]).unwrap().is_empty());
    let loaded = s.assets_for_cases(&[d.id.clone()]).unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(thumbnail.exists());
    assert_eq!(loaded[0].id, asset.id);
    assert!(s.assets_for_cases(&vec![d.id.clone(); 121]).is_err());
    s.connection
        .lock()
        .unwrap()
        .execute("UPDATE cases SET deleted_at=1 WHERE id=?1", [&d.id])
        .unwrap();
    assert!(s.assets_for_cases(&[d.id]).unwrap().is_empty());
}

#[test]
fn rename_to_pending_name_binds_once_and_preserves_earliest_draft_position() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path());
    let project = s.catalog_create("projects", "项目 A").unwrap();
    let tag = s.catalog_create("tags", "A").unwrap();
    let sessions = EditSessions::default();
    let mut drafts = vec![];
    // The target name precedes/follows the known ID, or exists only as pending text.
    for tags in [
        vec!["A", "B", "旁项"],
        vec!["B", "旁项", "A"],
        vec!["旁项", "B"],
    ] {
        let r = sessions
            .begin_persistent(&s, None, None, false, sessions.epoch())
            .unwrap();
        let draft = r.draft_id.clone().unwrap();
        let mut d = r.document;
        d.project = "项目 B".into();
        d.tags = tags.into_iter().map(str::to_owned).collect();
        sessions.stash(&s, &r.session_id, &d, 1).unwrap();
        sessions.clear();
        drafts.push(draft);
    }
    s.catalog_rename("tags", &tag, "B").unwrap();
    s.catalog_rename("projects", &project, "项目 B").unwrap();
    for (index, draft) in drafts.iter().enumerate() {
        let loaded = s.draft_document(draft).unwrap().0;
        assert_eq!(loaded.project, "项目 B");
        assert_eq!(
            loaded.tags,
            if index < 2 {
                vec!["B", "旁项"]
            } else {
                vec!["旁项", "B"]
            }
        );
        let c = s.connection.lock().unwrap();
        let (count, position): (i64, i64) = c
            .query_row(
                "SELECT count(*),min(position) FROM draft_tags WHERE draft_id=?1 AND tag_id=?2",
                params![draft, tag],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(position, if index < 2 { 0 } else { 1 });
        let pending:i64=c.query_row("SELECT count(*) FROM draft_pending_tags WHERE draft_id=?1 AND lower(trim(name))='b'",[draft],|r|r.get(0)).unwrap();
        assert_eq!(pending, 0);
        let project_id: String = c
            .query_row("SELECT project_id FROM drafts WHERE id=?1", [draft], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(project_id, project);
    }
    s.catalog_delete("tags", &[tag]).unwrap();
    s.catalog_delete("projects", &[project]).unwrap();
    for draft in drafts {
        let loaded = s.draft_document(&draft).unwrap().0;
        assert_eq!(loaded.tags, vec!["旁项"]);
        assert!(loaded.project.is_empty());
    }
}
