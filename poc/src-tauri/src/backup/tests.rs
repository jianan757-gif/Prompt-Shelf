use super::*;
use crate::{
    cases::{CaseDocument, EditSessions, ImageReference},
    paths::DataPaths,
};
use std::{collections::BTreeMap, io::Cursor};

fn open(root: &Path) -> Store {
    Store::open(DataPaths::new(root).unwrap()).unwrap()
}
fn source(root: &Path, color: u8) -> PathBuf {
    let path = root.join(format!("{color}.png"));
    image::RgbaImage::from_pixel(12, 8, image::Rgba([color, 90, 150, 128]))
        .save(&path)
        .unwrap();
    path
}
fn reference(id: &str) -> ImageReference {
    ImageReference {
        id: uuid::Uuid::new_v4().to_string(),
        asset_id: id.into(),
        original_name: "image.png".into(),
    }
}
fn case(s: &Store, title: &str, asset: &str) -> CaseDocument {
    let mut d = s.edit_document(None).unwrap();
    d.title = title.into();
    d.prompt = "complete prompt\n原始正文第二行\nreference order stays explicit".into();
    d.source_url = "https://example.com/source".into();
    d.project = "project".into();
    d.tags = vec!["tag".into()];
    d.references = vec![reference(asset), reference(asset)];
    s.commit_case(&d).unwrap();
    s.edit_document(Some(&d.id)).unwrap()
}
fn export(s: &Store, path: &Path) -> Manifest {
    s.export_backup(path, &JobControl::new("export"), |_| {})
        .unwrap()
}
fn contents(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut files = BTreeMap::new();
    for n in 0..zip.len() {
        let mut entry = zip.by_index(n).unwrap();
        let mut bytes = vec![];
        entry.read_to_end(&mut bytes).unwrap();
        files.insert(entry.name().to_owned(), bytes);
    }
    files
}
fn package(path: &Path, files: &BTreeMap<String, Vec<u8>>) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in files {
        zip.start_file(name, zip_options(bytes.len() as u64))
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}
fn edit_manifest(files: &mut BTreeMap<String, Vec<u8>>, change: impl FnOnce(&mut Manifest)) {
    let mut m: Manifest = serde_json::from_slice(&files["manifest.json"]).unwrap();
    change(&mut m);
    files.insert("manifest.json".into(), serde_json::to_vec(&m).unwrap());
}
fn replace_db(files: &mut BTreeMap<String, Vec<u8>>, change: impl FnOnce(&Connection)) {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("library.db");
    std::fs::write(&db, &files["library.db"]).unwrap();
    let c = Connection::open(&db).unwrap();
    change(&c);
    drop(c);
    let bytes = std::fs::read(db).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let size = bytes.len() as u64;
    files.insert("library.db".into(), bytes);
    edit_manifest(files, |m| {
        let r = m.files.iter_mut().find(|r| r.path == "library.db").unwrap();
        r.bytes = size;
        r.sha256 = hash;
    });
}
fn empty_fixture(root: &Path) -> (Store, PathBuf) {
    let s = open(root);
    let zip = root.join("valid.zip");
    export(&s, &zip);
    (s, zip)
}

#[test]
fn round_trip_online_snapshot_carries_live_trash_draft_config_and_unique_originals_without_online_cleanup(
) {
    let root = tempfile::tempdir().unwrap();
    let images = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let s = open(root.path());
    let shared = s.import_image(&source(images.path(), 20)).unwrap();
    let draft_asset = s.import_image(&source(images.path(), 40)).unwrap();
    let orphan = s.import_image(&source(images.path(), 60)).unwrap();
    let live = case(&s, "live", &shared.id);
    let trash = case(&s, "trash", &shared.id);
    s.soft_delete_case(&trash.id).unwrap();
    let sessions = EditSessions::default();
    let receipt = sessions
        .begin_persistent(&s, None, None, false, sessions.epoch())
        .unwrap();
    let draft_id = receipt.draft_id.clone().unwrap();
    let mut document = receipt.document;
    document.title = "draft".into();
    document.prompt = "dirty draft body\n尚未正式保存".into();
    document.project = "pending draft project".into();
    document.tags = vec!["tag".into(), "pending draft tag".into()];
    document.source_url = "https://example.com/draft".into();
    document.result = Some(reference(&shared.id));
    document.references = vec![reference(&draft_asset.id), reference(&draft_asset.id)];
    sessions
        .stash(&s, &receipt.session_id, &document, 1)
        .unwrap();
    s.set_quick_tag(&s.library_index().unwrap().tags[0].id, true)
        .unwrap();
    s.set_ui_preferences(serde_json::json!({"railWidth":260,"quickIntervalMs":350}))
        .unwrap();
    s.save_dictionary(vec![vec!["one".into(), "two".into()]])
        .unwrap();
    let live_document_before =
        serde_json::to_value(s.edit_document(Some(&live.id)).unwrap()).unwrap();
    let draft_document_before = serde_json::to_value(s.draft_document(&draft_id).unwrap()).unwrap();
    let before = s.check().unwrap();
    assert!(
        std::fs::metadata(root.path().join("library.db-wal"))
            .unwrap()
            .len()
            > 0,
        "the snapshot must include committed WAL state, not a raw main-file copy"
    );
    let path = root.path().join("backup.zip");
    let mut progress = vec![];
    let manifest = s
        .export_backup(&path, &JobControl::new("round-trip"), |event| {
            progress.push(event)
        })
        .unwrap();
    assert_eq!(
        manifest.summary,
        Summary {
            cases: 1,
            trash: 1,
            tags: 1,
            projects: 1,
            drafts: 1
        }
    );
    assert_eq!(manifest.files.len(), 3);
    assert_eq!(before, s.check().unwrap());
    assert!(Path::new(&orphan.original).exists());
    assert_eq!(s.check().unwrap()["assets"], 3);
    let files = contents(&path);
    assert_eq!(files.len(), 4);
    assert!(!files
        .keys()
        .any(|path| path.contains("cache") || path.contains("staging") || path.contains("logs")));
    for record in &manifest.files {
        assert_eq!(record.bytes, files[&record.path].len() as u64);
        assert_eq!(
            record.sha256,
            format!("{:x}", Sha256::digest(&files[&record.path]))
        );
    }
    assert!(progress.iter().any(|p| p.phase == "snapshot"));
    assert_eq!(progress.last().unwrap().phase, "completed");
    assert_eq!(
        progress.last().unwrap().bytes_done,
        progress.last().unwrap().bytes_total
    );
    let ready = prepare_restore(&path, parent.path(), &JobControl::new("restore"), |_| {}).unwrap();
    assert_eq!(ready.summary(), &manifest.summary);
    let installed = ready.into_storage().unwrap();
    assert!(uuid::Uuid::parse_str(installed.file_name().unwrap().to_str().unwrap()).is_ok());
    let restored = open(&installed);
    assert_eq!(restored.check().unwrap()["assets"], 2);
    assert_eq!(restored.trash_index().unwrap()[0].case.id, trash.id);
    assert_eq!(restored.library_index().unwrap().cases[0].id, live.id);
    assert_eq!(
        serde_json::to_value(restored.edit_document(Some(&live.id)).unwrap()).unwrap(),
        live_document_before,
        "formal fields, source, categories and stable image occurrence order round-trip"
    );
    assert_eq!(
        serde_json::to_value(restored.draft_document(&draft_id).unwrap()).unwrap(),
        draft_document_before,
        "dirty draft fields, pending categories, result/reference IDs and save sequence round-trip"
    );
    assert_eq!(restored.draft_summaries().unwrap().len(), 1);
    assert_eq!(restored.ui_preferences().unwrap()["railWidth"], 260);
    assert_eq!(restored.dictionary().unwrap(), vec![vec!["one", "two"]]);
    assert!(restored.library_index().unwrap().tags[0].is_quick);
}

#[test]
fn existing_destination_and_cancelled_export_do_not_publish_or_replace() {
    let root = tempfile::tempdir().unwrap();
    let s = open(root.path());
    let destination = root.path().join("existing.zip");
    std::fs::write(&destination, b"keep me").unwrap();
    assert!(s
        .export_backup(&destination, &JobControl::new("existing"), |_| {})
        .is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"keep me");
    let control = JobControl::new("cancel");
    let cancelled = root.path().join("cancelled.zip");
    let result = s.export_backup(&cancelled, &control, |event| {
        if event.phase == "archive" {
            control.cancel();
        }
    });
    assert_eq!(result.unwrap_err(), "backup_cancelled");
    assert!(!cancelled.exists());
    assert_eq!(std::fs::read_dir(s.paths().staging()).unwrap().count(), 0);
}

#[test]
fn publishing_has_crossed_the_atomic_cancel_boundary_before_final_file_appears() {
    let root = tempfile::tempdir().unwrap();
    let store = open(root.path());
    let destination = root.path().join("published.zip");
    let control = JobControl::new("publish-boundary");
    let mut cancel_accepted = None;
    store
        .export_backup(&destination, &control, |event| {
            if event.phase == "publishing" {
                assert!(!destination.exists());
                cancel_accepted = Some(control.cancel());
            }
        })
        .unwrap();
    assert_eq!(cancel_accepted, Some(false));
    assert!(contents(&destination).contains_key("manifest.json"));
}

#[test]
fn restore_rejects_missing_hash_mismatch_future_unknown_schema_and_summary_without_touching_live() {
    let root = tempfile::tempdir().unwrap();
    let images = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let s = open(root.path());
    let image = s.import_image(&source(images.path(), 50)).unwrap();
    case(&s, "live", &image.id);
    let path = root.path().join("valid.zip");
    export(&s, &path);
    let original = contents(&path);
    let before = s.check().unwrap();
    for kind in [
        "missing",
        "hash",
        "future",
        "summary",
        "structure",
        "reference",
        "dimensions",
        "unknown-setting",
    ] {
        let mut files = original.clone();
        match kind {
            "missing" => {
                let name = files
                    .keys()
                    .find(|n| n.starts_with("assets/"))
                    .unwrap()
                    .clone();
                files.remove(&name);
            }
            "hash" => edit_manifest(&mut files, |m| {
                m.files
                    .iter_mut()
                    .find(|r| r.path.starts_with("assets/"))
                    .unwrap()
                    .sha256 = "0".repeat(64)
            }),
            "future" => edit_manifest(&mut files, |m| m.schema_version = SCHEMA_VERSION + 1),
            "summary" => edit_manifest(&mut files, |m| m.summary.cases += 1),
            "structure" => {
                replace_db(&mut files, |c| {
                    c.execute_batch("CREATE TRIGGER unrelated AFTER INSERT ON cases BEGIN DELETE FROM tags; END;").unwrap()
                })
            }
            "reference" => replace_db(&mut files, |c| {
                c.pragma_update(None, "foreign_keys", false).unwrap();
                c.execute("UPDATE case_assets SET asset_id='missing'", [])
                    .unwrap();
            }),
            "dimensions" => replace_db(&mut files, |c| {
                c.execute_batch("DROP TRIGGER assets_immutable").unwrap();
                c.execute("UPDATE assets SET width=999", []).unwrap();
                c.execute_batch("CREATE TRIGGER assets_immutable BEFORE UPDATE OF sha256,extension,mime,byte_size,width,height,relative_path ON assets\n BEGIN SELECT RAISE(ABORT,'immutable_asset'); END;").unwrap();
            }),
            "unknown-setting" => replace_db(&mut files, |c| {
                c.execute(
                    "INSERT INTO settings VALUES('machine_path','\"C:/private\"')",
                    [],
                )
                .unwrap();
            }),
            _ => unreachable!(),
        }
        let invalid = root.path().join(format!("{kind}.zip"));
        package(&invalid, &files);
        let error = prepare_restore(&invalid, parent.path(), &JobControl::new(kind), |_| {})
            .unwrap_err_string();
        let expected = match kind {
            "missing" => "backup_file_missing",
            "hash" => "backup_hash_mismatch",
            "future" => "backup_future_or_unknown_schema",
            "summary" => "backup_summary_mismatch",
            "structure" => "backup_database_structure_invalid",
            "reference" => "backup_foreign_keys_failed",
            "dimensions" => "backup_image_dimensions_mismatch",
            "unknown-setting" => "backup_settings_not_supported",
            _ => unreachable!(),
        };
        assert!(error.contains(expected), "{kind}: {error}");
        assert_eq!(
            std::fs::read_dir(parent.path()).unwrap().count(),
            0,
            "{kind} leaves no staging"
        );
        assert_eq!(s.check().unwrap(), before);
    }
}

#[test]
fn restore_rejects_corrupt_zip_traversal_absolute_case_alias_and_symlink_entries() {
    let root = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let (_s, path) = empty_fixture(root.path());
    let original = contents(&path);
    let corrupt = root.path().join("corrupt.zip");
    std::fs::write(&corrupt, b"not a ZIP").unwrap();
    assert!(prepare_restore(&corrupt, parent.path(), &JobControl::new("corrupt"), |_| {}).is_err());
    for name in [
        "../escape",
        "/absolute",
        "C:/absolute",
        "assets/../escape",
        "Library.db",
        "assets\\escape.png",
    ] {
        let mut files = original.clone();
        files.insert(name.into(), b"x".to_vec());
        let invalid = root.path().join("bad-path.zip");
        package(&invalid, &files);
        assert!(
            prepare_restore(&invalid, parent.path(), &JobControl::new("path"), |_| {}).is_err(),
            "{name}"
        );
    }
    let linked = root.path().join("symlink.zip");
    let mut writer = ZipWriter::new(File::create(&linked).unwrap());
    writer
        .add_symlink("library.db", "outside", SimpleFileOptions::default())
        .unwrap();
    writer.finish().unwrap();
    assert!(
        prepare_restore(&linked, parent.path(), &JobControl::new("symlink"), |_| {})
            .unwrap_err_string()
            .contains("backup_entry_type_not_supported")
    );
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
    assert!(!root.path().parent().unwrap().join("escape").exists());
}

trait ErrorString {
    fn unwrap_err_string(self) -> String;
}
impl ErrorString for Result<PreparedRestore, String> {
    fn unwrap_err_string(self) -> String {
        match self {
            Ok(_) => panic!("expected package failure"),
            Err(error) => error,
        }
    }
}

#[test]
fn duplicate_central_entry_is_not_hidden_by_zip_index_coalescing() {
    let root = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let (_s, path) = empty_fixture(root.path());
    let mut files = contents(&path);
    files.insert("library.dz".into(), b"extra".to_vec());
    let duplicate = root.path().join("duplicate.zip");
    package(&duplicate, &files);
    let mut raw = std::fs::read(&duplicate).unwrap();
    for offset in 0..raw.len().saturating_sub(10) {
        if &raw[offset..offset + 10] == b"library.dz" {
            raw[offset + 9] = b'b'
        }
    }
    std::fs::write(&duplicate, raw).unwrap();
    assert_eq!(
        prepare_restore(
            &duplicate,
            parent.path(),
            &JobControl::new("duplicate"),
            |_| {}
        )
        .unwrap_err_string(),
        "backup_duplicate_entry"
    );
}

#[test]
fn restored_v1_runs_only_application_migration_in_temporary_database() {
    let root = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let path = root.path().join("v1.db");
    let c = Connection::open(&path).unwrap();
    c.execute_batch(include_str!("../migrations/001_initial.sql"))
        .unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    let bytes = std::fs::read(path).unwrap();
    let manifest = Manifest {
        format: "prompt-shelf-backup".into(),
        version: 1,
        schema_version: 1,
        app_version: "0.1.0".into(),
        app_revision: "4.4-a".into(),
        created_at: 1,
        summary: Summary::default(),
        files: vec![FileRecord {
            path: "library.db".into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        }],
    };
    let zip = root.path().join("v1.zip");
    package(
        &zip,
        &BTreeMap::from([
            ("library.db".into(), bytes),
            (
                "manifest.json".into(),
                serde_json::to_vec(&manifest).unwrap(),
            ),
        ]),
    );
    let ready = prepare_restore(&zip, parent.path(), &JobControl::new("v1"), |_| {}).unwrap();
    let c = Connection::open(ready.path().join("library.db")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn restore_space_and_streamed_actual_bytes_are_bounded_and_cancellation_cleans_preview() {
    let root = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let (_s, path) = empty_fixture(root.path());
    assert_eq!(
        prepare_with_space(
            &path,
            parent.path(),
            &JobControl::new("space"),
            |_| {},
            Some(0)
        )
        .unwrap_err_string(),
        "backup_insufficient_space"
    );
    let control = JobControl::new("cancel");
    let result = prepare_restore(&path, parent.path(), &control, |event| {
        if event.bytes_done > 0 || event.phase == "validate" {
            control.cancel();
        }
    });
    assert_eq!(result.unwrap_err_string(), "backup_cancelled");
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
    let control = JobControl::new("actual");
    let mut tracker = Tracker::new(&control, |_| {}, "extract", 2, 1);
    let mut output = vec![];
    assert_eq!(
        copy_hash(
            &mut Cursor::new(vec![0u8; 10]),
            &mut output,
            2,
            &mut tracker
        )
        .unwrap_err(),
        "backup_actual_bytes_exceed_manifest"
    );
    assert!(output.is_empty());
}

#[test]
fn zip64_entry_option_writes_zip64_local_header_without_allocating_multi_gigabyte_fixture() {
    // This is a targeted ZIP64 format/options test, not a claimed physical 4 GiB stress run.
    let mut zip = ZipWriter::new(Cursor::new(vec![]));
    zip.start_file("large", zip_options(zip::ZIP64_BYTES_THR + 1))
        .unwrap();
    zip.write_all(b"small payload").unwrap();
    let data = zip.finish().unwrap().into_inner();
    assert!(data.windows(4).any(|value| value == [1, 0, 16, 0]));
    let mut read = ZipArchive::new(Cursor::new(data)).unwrap();
    let mut output = String::new();
    read.by_name("large")
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    assert_eq!(output, "small payload");
}

#[test]
fn cancellation_and_commit_have_one_atomic_winner() {
    let cancelled = JobControl::new("cancel-first");
    assert!(cancelled.cancel());
    assert!(
        cancelled.cancel(),
        "repeated cancellation remains acknowledged"
    );
    assert_eq!(cancelled.begin_commit().unwrap_err(), "backup_cancelled");
    assert_eq!(cancelled.check().unwrap_err(), "backup_cancelled");

    let committing = JobControl::new("commit-first");
    committing.begin_commit().unwrap();
    assert!(
        !committing.cancel(),
        "commit is the noncancellable boundary"
    );
    assert!(!committing.cancel());
    assert!(committing.check().is_ok());
    assert_eq!(
        committing.begin_commit().unwrap_err(),
        "backup_commit_already_started"
    );
}
