use super::*;
use image::{ImageDecoder, ImageEncoder};

#[test]
fn readonly_projection_exposes_existing_dates_source_and_catalog_state(){
    let directory=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();
    let store=store(directory.path());store.import_manifest(&fixture(source.path(),false)).unwrap();
    {let connection=store.connection.lock().unwrap();
     connection.execute("UPDATE cases SET source_url='https://example.com/reference',created_at=1000,updated_at=2000,last_used_at=3000",[]).unwrap();
     connection.execute("UPDATE tags SET is_quick=1,quick_order=4,created_at=4000",[]).unwrap();}
    let library=store.library().unwrap();assert_eq!(library.cases[0].source_url,"https://example.com/reference");
    assert_eq!(library.cases[0].created_at,Some(1000));assert_eq!(library.cases[0].last_used_at,Some(3000));
    assert_eq!(library.tags.len(),1);assert!(library.tags[0].is_quick);assert_eq!(library.tags[0].quick_order,4);assert_eq!(library.tags[0].created_at,4000);
    assert_eq!(library.projects.len(),1);
    store.connection.lock().unwrap().execute("UPDATE cases SET deleted_at=5000 WHERE id=(SELECT id FROM cases LIMIT 1)",[]).unwrap();
    assert_eq!(store.library().unwrap().trash_count,1);
}

fn store(directory: &Path) -> Store {
    Store::open(DataPaths::new(directory).unwrap()).unwrap()
}
fn png(path: &Path) {
    image::RgbaImage::from_pixel(400, 300, image::Rgba([80, 120, 160, 96]))
        .save(path)
        .unwrap();
}
fn fixture(root: &Path, broken_second: bool) -> std::path::PathBuf {
    std::fs::create_dir_all(root).unwrap();
    png(&root.join("原图.png"));
    png(&root.join("旧缩略.png"));
    let mut input = serde_json::json!({"schema_version":1,"assets":[{"id":"a","original":"原图.png","thumb":"旧缩略.png","width":1,"height":1,"sha256":assets::hash_file(&root.join("原图.png")).unwrap()}],"cases":[
        {"id":"c1","title":"共享案例一","prompt":"完整第一行\n第二行","project":"项目","tags":["产品","产品"],"favorite":true,"used":2,"result":"a","refs":["a","a"]},
        {"id":"c2","title":"共享案例二","prompt":"另一提示词","project":" 项目 ","tags":["产品"],"favorite":false,"used":0,"result":null,"refs":["a"]}]});
    if broken_second {
        std::fs::write(root.join("坏图.png"), b"not an image").unwrap();
        input["assets"].as_array_mut().unwrap().push(serde_json::json!({"id":"broken","original":"坏图.png","thumb":"旧缩略.png","width":2,"height":2}));
        input["cases"][1]["refs"] = serde_json::json!(["a", "broken"]);
    }
    let path = root.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    path
}
#[test]
fn empty_library_reopens_and_configuration_is_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let first = store(directory.path());
    assert_eq!(first.counts().unwrap(), (0, 0));
    assert!(first.library().unwrap().cases.is_empty());
    let c = first.connection.lock().unwrap();
    assert_eq!(
        c.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    assert_eq!(
        c.pragma_query_value(None, "synchronous", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        12
    );
    drop(c);
    drop(first);
    assert_eq!(store(directory.path()).check().unwrap()["cases"], 0);
}
#[test]
fn shared_original_references_uuid_dtos_and_reopen_are_lossless() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let manifest = fixture(source.path(), false);
    let original = assets::hash_file(&source.path().join("原图.png")).unwrap();
    let first = store(directory.path());
    first.import_manifest(&manifest).unwrap();
    assert_eq!(first.counts().unwrap(), (2, 1));
    let library = first.library().unwrap();
    assert!(library
        .cases
        .iter()
        .all(|c| uuid::Uuid::parse_str(&c.id).is_ok()));
    let one = library
        .cases
        .iter()
        .find(|c| c.title == "共享案例一")
        .unwrap();
    assert_eq!(one.prompt, "完整第一行\n第二行");
    assert_eq!(one.refs.len(), 2);
    assert_eq!(one.refs[0], one.refs[1]);
    assert_eq!(one.result.as_ref(), Some(&one.refs[0]));
    assert_eq!(one.tags, vec!["产品"]);
    assert_eq!(first.check().unwrap()["caseAssetReferences"], 4);
    assert_eq!(
        std::fs::read_dir(directory.path().join("assets"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        assets::hash_file(Path::new(&library.assets[0].original)).unwrap(),
        original
    );
    assert_eq!(
        assets::hash_file(&source.path().join("原图.png")).unwrap(),
        original
    );
    assert_eq!(
        (library.assets[0].width, library.assets[0].height),
        (400, 300)
    );
    let thumb = image::open(&library.assets[0].thumbnail)
        .unwrap()
        .to_rgba8();
    assert_eq!(thumb.height(), 180);
    assert_eq!(thumb.width(), 240);
    assert_eq!(thumb.get_pixel(50, 50).0[3], 96);
    assert_ne!(library.assets[0].thumbnail, library.assets[0].original);
    let ids = library
        .cases
        .iter()
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    drop(first);
    let second = store(directory.path());
    assert_eq!(
        second
            .library()
            .unwrap()
            .cases
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert!(second
        .import_manifest(&manifest)
        .unwrap_err()
        .contains("empty_library"));
}
#[test]
fn foreign_keys_results_and_draft_protection_enforced_by_sqlite() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store = store(directory.path());
    store
        .import_manifest(&fixture(source.path(), false))
        .unwrap();
    let library = store.library().unwrap();
    let case = &library.cases[0].id;
    let asset = &library.assets[0].id;
    let c = store.connection.lock().unwrap();
    assert!(c
        .execute("DELETE FROM assets WHERE id=?1", [asset])
        .is_err());
    assert!(c
        .execute(
            "UPDATE assets SET relative_path='cache/thumbs/x.png' WHERE id=?1",
            [asset]
        )
        .is_err());
    assert!(c
        .execute(
            "INSERT INTO case_assets VALUES('missing',?1,'absent','reference',99,'x')",
            [case]
        )
        .is_err());
    c.execute(
        "INSERT INTO drafts(id,case_id,updated_at) VALUES('draft',?1,1)",
        [case],
    )
    .unwrap();
    assert!(c.execute("DELETE FROM cases WHERE id=?1", [case]).is_err());
    c.execute(
        "INSERT INTO draft_assets VALUES('dr','draft',?1,'result',0,'x')",
        [asset],
    )
    .unwrap();
    assert!(c
        .execute(
            "INSERT INTO draft_assets VALUES('dr2','draft',?1,'result',1,'x')",
            [asset]
        )
        .is_err());
    c.execute("DELETE FROM cases WHERE id<>?1", [case]).unwrap();
    c.execute("DELETE FROM case_assets", []).unwrap();
    assert!(c.execute("DELETE FROM assets", []).is_err());
    c.execute("DELETE FROM projects", []).unwrap();
    assert_eq!(
        c.query_row("SELECT project_id IS NULL FROM cases", [], |r| r
            .get::<_, bool>(0))
            .unwrap(),
        true
    );
    c.execute("DELETE FROM tags", []).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM case_tags", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn failed_image_batch_never_commits_partial_cases_or_assets() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store = store(directory.path());
    assert!(store
        .import_manifest(&fixture(source.path(), true))
        .is_err());
    assert_eq!(store.counts().unwrap(), (0, 0));
    assert_eq!(
        std::fs::read_dir(directory.path().join("staging"))
            .unwrap()
            .count(),
        0
    );
    drop(store);
    assert_eq!(self::store(directory.path()).counts().unwrap(), (0, 0));
}
#[test]
fn failed_sql_batch_rolls_back_after_ready_files_and_prior_case_inserts() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let manifest = fixture(source.path(), false);
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    json["cases"][1]["prompt"] = serde_json::json!("");
    std::fs::write(&manifest, serde_json::to_vec(&json).unwrap()).unwrap();
    let store = store(directory.path());
    assert!(store.import_manifest(&manifest).is_err());
    assert_eq!(store.counts().unwrap(), (0, 0));
    let c = store.connection.lock().unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM tags", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn paths_reject_absolute_traversal_cache_original_and_bad_hash() {
    let directory = tempfile::tempdir().unwrap();
    let paths = DataPaths::new(directory.path()).unwrap();
    assert!(paths.resolve("../elsewhere", "assets").is_err());
    assert!(paths
        .resolve(&directory.path().to_string_lossy(), "assets")
        .is_err());
    assert!(paths.resolve("cache/thumbs/x.png", "assets").is_err());
    assert!(DataPaths::asset_relative("../x", "png").is_err());
    let source = tempfile::tempdir().unwrap();
    let manifest = fixture(source.path(), false);
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    json["assets"][0]["original"] = serde_json::json!("../escape.png");
    std::fs::write(&manifest, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(store(directory.path()).import_manifest(&manifest).is_err());
}
#[test]
fn future_or_unversioned_database_is_not_overwritten_and_failed_migration_rolls_back() {
    let directory = tempfile::tempdir().unwrap();
    let paths = DataPaths::new(directory.path()).unwrap();
    let c = Connection::open(paths.database().unwrap()).unwrap();
    c.execute_batch("CREATE TABLE precious(value TEXT); INSERT INTO precious VALUES('keep'); PRAGMA user_version=999;").unwrap();
    drop(c);
    let before = assets::hash_file(&paths.database().unwrap()).unwrap();
    assert!(Store::open(paths.clone())
        .err()
        .unwrap()
        .contains("future_schema"));
    assert_eq!(
        assets::hash_file(&paths.database().unwrap()).unwrap(),
        before
    );
    let mut c = Connection::open(paths.database().unwrap()).unwrap();
    c.pragma_update(None, "user_version", 0).unwrap();
    assert!(apply_initial(
        &mut c,
        "CREATE TABLE transient(value TEXT); INSERT INTO absent VALUES(1);"
    )
    .is_err());
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT value FROM precious", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "keep"
    );
    assert!(c.prepare("SELECT * FROM transient").is_err());
    drop(c);
    assert!(Store::open(paths)
        .err()
        .unwrap()
        .contains("unversioned_nonempty"));
}
#[test]
fn store_lifetime_lock_excludes_second_owner_and_releases_on_drop() {
    let directory = tempfile::tempdir().unwrap();
    let first = store(directory.path());
    assert!(Store::open(DataPaths::new(directory.path()).unwrap())
        .err()
        .unwrap()
        .contains("library_in_use"));
    drop(first);
    assert_eq!(store(directory.path()).counts().unwrap(), (0, 0));
}
#[test]
fn missing_or_corrupt_thumbnails_rebuild_only_from_matching_original() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    png(&source.path().join("one.png"));
    let store = store(directory.path());
    let asset = store.import_image(&source.path().join("one.png")).unwrap();
    let original = assets::hash_file(Path::new(&asset.original)).unwrap();
    std::fs::remove_file(&asset.thumbnail).unwrap();
    store.library().unwrap();
    assert!(image::open(&asset.thumbnail).is_ok());
    std::fs::write(&asset.thumbnail, b"corrupt png").unwrap();
    store.library().unwrap();
    assert!(image::open(&asset.thumbnail).is_ok());
    assert_eq!(
        assets::hash_file(Path::new(&asset.original)).unwrap(),
        original
    );
    std::fs::remove_file(&asset.thumbnail).unwrap();
    std::fs::write(&asset.original, b"damaged original").unwrap();
    assert!(store
        .library()
        .err()
        .unwrap()
        .contains("immutable_asset_hash_mismatch"));
    assert!(!Path::new(&asset.thumbnail).exists());
}
#[test]
fn jpeg_exif_orientation_and_icc_are_preserved_in_independent_preview() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("orientation.jpg");
    let pixels = image::RgbImage::from_pixel(40, 20, image::Rgb([80, 120, 160]));
    let mut bytes = vec![];
    // Little endian TIFF, orientation tag 6 (90 degrees clockwise).
    let exif = vec![
        b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
    ];
    let mut profile = vec![0u8; 128];
    profile[0..4].copy_from_slice(&128u32.to_be_bytes());
    profile[36..40].copy_from_slice(b"acsp");
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut bytes);
    encoder.set_exif_metadata(exif).unwrap();
    encoder.set_icc_profile(profile.clone()).unwrap();
    encoder
        .write_image(pixels.as_raw(), 40, 20, image::ExtendedColorType::Rgb8)
        .unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let asset = store(directory.path()).import_image(&path).unwrap();
    assert_eq!((asset.width, asset.height), (20, 40));
    assert_eq!(std::fs::read(&asset.original).unwrap(), bytes);
    let mut decoder = image::ImageReader::open(&asset.thumbnail)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap();
    assert_eq!(decoder.icc_profile().unwrap(), Some(profile));
    assert_eq!(decoder.dimensions(), (20, 40));
}
