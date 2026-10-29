use prompt_shelf_poc::{paths::DataPaths, scope, store::Store};
use std::path::Path;
use tauri::Manager;
fn store(path: &Path) -> Store {
    Store::open(DataPaths::new(path).unwrap()).unwrap()
}
fn png(path: &Path) {
    image::RgbaImage::from_pixel(400, 300, image::Rgba([80, 120, 160, 96]))
        .save(path)
        .unwrap();
}
#[test]
fn incremental_import_deduplicates_and_refresh_scope_grants_only_thumbnails() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    png(&source.path().join("one.png"));
    let store = store(directory.path());
    let app = tauri::test::mock_app();
    scope::authorize_thumbnails(app.handle(), &store, &store.library().unwrap()).unwrap();
    let first = store.import_image(&source.path().join("one.png")).unwrap();
    assert!(!app.asset_protocol_scope().is_allowed(&first.thumbnail));
    let same = store.import_image(&source.path().join("one.png")).unwrap();
    assert_eq!(same.id, first.id);
    assert_eq!(store.counts().unwrap(), (0, 1));
    let mut document=store.edit_document(None).unwrap();
    document.title="缩略图授权".into();document.prompt="刷新仅授权引用资产的缩略图".into();
    document.references=vec![prompt_shelf_poc::cases::ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:first.id.clone(),original_name:"one.png".into()}];
    store.commit_case(&document).unwrap();
    let refreshed = store.library().unwrap();
    scope::authorize_thumbnails(app.handle(), &store, &refreshed).unwrap();
    assert!(app.asset_protocol_scope().is_allowed(&first.thumbnail));
    assert!(!app.asset_protocol_scope().is_allowed(&first.original));
    let neighbor = directory.path().join("cache/thumbs/neighbor.png");
    png(&neighbor);
    assert!(!app.asset_protocol_scope().is_allowed(&neighbor));
    assert!(!app
        .asset_protocol_scope()
        .is_allowed(directory.path().join("library.db")));
    let mut forged = refreshed;
    forged.assets[0].thumbnail = first.original;
    assert!(scope::authorize_thumbnails(app.handle(), &store, &forged).is_err());
}

mod retirement_tests{
    use prompt_shelf_poc::{store::Store,scope::ThumbnailGrants};
    use tauri::Manager;
    use prompt_shelf_poc::{cases::ImageReference,paths::DataPaths};
    #[test]fn retiring_old_grants_preserves_new_library_granted_before_reinitialized_event(){
        let old=tempfile::tempdir().unwrap();let new=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let app=tauri::test::mock_app();let grants=ThumbnailGrants::default();
        let mut snapshots=vec![];
        for (index,root) in [old.path(),new.path()].into_iter().enumerate(){let store=Store::open(DataPaths::new(root).unwrap()).unwrap();let file=source.path().join(format!("{index}.png"));image::RgbaImage::from_pixel(10,8,image::Rgba([index as u8,80,160,255])).save(&file).unwrap();let asset=store.import_image(&file).unwrap();let mut document=store.edit_document(None).unwrap();document.title="scope".into();document.prompt="p".into();document.result=Some(ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:asset.id.clone(),original_name:"image.png".into()});store.commit_case(&document).unwrap();let library=store.library().unwrap();grants.authorize(app.handle(),&store,&library).unwrap();snapshots.push(asset);}
        assert!(app.asset_protocol_scope().is_allowed(&snapshots[0].thumbnail));assert!(app.asset_protocol_scope().is_allowed(&snapshots[1].thumbnail));
        grants.revoke_previous(app.handle(),&dunce::canonicalize(new.path()).unwrap()).unwrap();
        assert!(!app.asset_protocol_scope().is_allowed(&snapshots[0].thumbnail));assert!(app.asset_protocol_scope().is_allowed(&snapshots[1].thumbnail));assert!(!app.asset_protocol_scope().is_allowed(&snapshots[1].original));
    }
}
