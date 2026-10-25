use prompt_shelf_poc::{assets, paths::DataPaths, store::Store};
use std::{path::Path, process::Command};

fn cli(directory: &Path, manifest: Option<&Path>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_prompt-shelf-store"));
    command.arg("--data-dir").arg(directory).arg("--check");
    if let Some(path) = manifest {
        command.arg("--import-manifest").arg(path);
    }
    command.output().unwrap()
}
#[test]
fn offline_real_process_import_exit_reopen_and_in_use_guard() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let first = cli(directory.path(), None);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&first.stdout).unwrap()["cases"],
        0
    );
    let pixels = image::RgbaImage::from_pixel(4, 3, image::Rgba([30, 80, 130, 120]));
    pixels.save(source.path().join("原图.png")).unwrap();
    pixels.save(source.path().join("thumb.png")).unwrap();
    let original_hash = assets::hash_file(&source.path().join("原图.png")).unwrap();
    let manifest = source.path().join("manifest.json");
    std::fs::write(&manifest,serde_json::to_vec(&serde_json::json!({"schema_version":1,"assets":[{"id":"a","original":"原图.png","thumb":"thumb.png","width":4,"height":3,"sha256":original_hash}],"cases":[{"id":"c","title":"进程重开测试","prompt":"第一行\n第二行","project":"","tags":[],"favorite":false,"used":0,"result":"a","refs":["a","a"]}]})).unwrap()).unwrap();
    let imported = cli(directory.path(), Some(&manifest));
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let checked = cli(directory.path(), None);
    assert!(checked.status.success());
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["cases"], 1);
    assert_eq!(report["assets"], 1);
    assert_eq!(report["caseAssetReferences"], 3);
    assert_eq!(report["originalHashes"], "ok");
    let store = Store::open(DataPaths::new(directory.path()).unwrap()).unwrap();
    let library = store.library().unwrap();
    assert_eq!(library.cases[0].prompt, "第一行\n第二行");
    assert_eq!(library.cases[0].refs[0], library.cases[0].refs[1]);
    assert_eq!(library.assets[0].sha256, original_hash);
    let blocked = cli(directory.path(), Some(&manifest));
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("library_in_use"));
    assert_eq!(store.counts().unwrap(), (1, 1));
    drop(store);
    assert!(cli(directory.path(), None).status.success());
}
#[test]
fn offline_cli_requires_explicit_directory_before_any_data_access() {
    let result = Command::new(env!("CARGO_BIN_EXE_prompt-shelf-store"))
        .arg("--check")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires_explicit_data_dir"));
}
