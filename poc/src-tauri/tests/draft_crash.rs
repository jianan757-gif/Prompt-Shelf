//! Only self-spawned test workers are killed; no GUI, clipboard, or user libraries.
use prompt_shelf_poc::{
    cases::{EditSessions, ImageReference},
    paths::DataPaths,
    store::Store,
};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[test]
fn draft_crash_child() {
    let Some(root) = std::env::var_os("PROMPT_SHELF_DRAFT_CRASH_CHILD") else {
        return;
    };
    let root = PathBuf::from(root);
    let s = Store::open(DataPaths::new(&root).unwrap()).unwrap();
    let source = root.join("isolated-source.png");
    image::RgbaImage::from_pixel(5, 4, image::Rgba([20, 40, 60, 120]))
        .save(&source)
        .unwrap();
    let asset = s.import_image(&source).unwrap();
    let mut a = s.edit_document(None).unwrap();
    a.title = "正式 A".into();
    a.prompt = "正式正文".into();
    a.references = vec![ImageReference {
        id: uuid::Uuid::new_v4().to_string(),
        asset_id: asset.id.clone(),
        original_name: "共享.png".into(),
    }];
    s.commit_case(&a).unwrap();
    let sessions = EditSessions::default();
    let r = sessions
        .begin_persistent(&s, Some(&a.id), None, false, sessions.epoch())
        .unwrap();
    let mut d = r.document;
    d.title = "现有案例草稿 B".into();
    d.project = "新项目未登记".into();
    d.tags = vec!["新标签未登记".into()];
    d.references.push(ImageReference {
        id: uuid::Uuid::new_v4().to_string(),
        asset_id: asset.id,
        original_name: "重复引用.png".into(),
    });
    sessions.stash(&s, &r.session_id, &d, 1).unwrap();
    sessions.clear();
    let r = sessions
        .begin_persistent(&s, None, None, false, sessions.epoch())
        .unwrap();
    let mut new = r.document;
    new.prompt = "没有标题的新建草稿".into();
    new.result = d.references.first().cloned();
    new.result.as_mut().unwrap().id = uuid::Uuid::new_v4().to_string();
    sessions.stash(&s, &r.session_id, &new, 1).unwrap();
    std::fs::write(
        root.join("worker-ready"),
        serde_json::to_vec(
            &serde_json::json!({"caseId":a.id,"assetPath":asset.original,"hash":asset.sha256}),
        )
        .unwrap(),
    )
    .unwrap();
    loop {
        std::thread::park_timeout(Duration::from_secs(60));
    }
}
#[test]
fn killed_worker_reopens_existing_and_new_drafts_without_formal_changes() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "draft_crash_child", "--nocapture"])
        .env("PROMPT_SHELF_DRAFT_CRASH_CHILD", root.path())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(root.path().join("worker-stderr.txt")).unwrap())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !root.path().join("worker-ready").exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!(
                "isolated child exited early: {status}: {}",
                std::fs::read_to_string(root.path().join("worker-stderr.txt")).unwrap()
            )
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("isolated worker timeout")
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("worker-ready")).unwrap()).unwrap();
    let s = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
    let summary = s.draft_summaries().unwrap();
    assert_eq!(summary.len(), 2);
    let library = s.library().unwrap();
    assert_eq!(library.cases.len(), 1);
    assert_eq!(library.cases[0].title, "正式 A");
    assert_eq!(library.cases[0].refs.len(), 1);
    assert!(library.tags.is_empty());
    assert!(library.projects.is_empty());
    assert_eq!(s.check().unwrap()["integrity"], "ok");
    let sessions = EditSessions::default();
    for draft in summary {
        let r = sessions
            .begin_persistent(&s, None, Some(&draft.id), false, sessions.epoch())
            .unwrap();
        if draft.case_id.is_some() {
            assert_eq!(r.document.title, "现有案例草稿 B");
            assert_eq!(r.document.project, "新项目未登记");
            assert_eq!(r.document.tags, vec!["新标签未登记"]);
            assert_eq!(r.document.references.len(), 2);
        } else {
            assert!(r.document.title.is_empty());
            assert_eq!(r.document.prompt, "没有标题的新建草稿");
            assert!(r.document.result.is_some());
        }
        sessions.discard(&s, &r.session_id).unwrap();
    }
    assert_eq!(
        prompt_shelf_poc::assets::hash_file(std::path::Path::new(
            metadata["assetPath"].as_str().unwrap()
        ))
        .unwrap(),
        metadata["hash"].as_str().unwrap()
    );
    assert_eq!(s.library().unwrap().cases[0].title, "正式 A");
    // The released B1 CLI must reject schema 2 before changing its database.
    let b1 = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target-stage-b1-business/release/prompt-shelf-store.exe");
    if cfg!(windows) && b1.exists() {
        let db = s.paths().database().unwrap();
        drop(s);
        let before = std::fs::read(&db).unwrap();
        let output = Command::new(b1)
            .arg("--data-dir")
            .arg(root.path())
            .arg("--check")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("future_schema: 2"));
        assert_eq!(std::fs::read(db).unwrap(), before);
    }
}
