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
