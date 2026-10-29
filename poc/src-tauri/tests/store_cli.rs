//! Real offline CLI subprocesses, with only test-owned temporary libraries.
use prompt_shelf_poc::{cases::{EditSessions,ImageReference},gc::ClipboardProtection,paths::DataPaths,store::Store};
use std::{path::{Path,PathBuf},process::{Command,Output}};

fn open(root:&Path)->Store{Store::open(DataPaths::new(root).unwrap()).unwrap()}
fn image(root:&Path,name:&str,color:u8)->PathBuf{let path=root.join(name);image::RgbaImage::from_pixel(12,8,image::Rgba([color,80,120,96])).save(&path).unwrap();path}
fn reference(id:&str)->ImageReference{ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:id.into(),original_name:"image.png".into()}}
fn cli(root:&Path)->Output{Command::new(env!("CARGO_BIN_EXE_prompt-shelf-store")).arg("--data-dir").arg(root).arg("--check").output().unwrap()}
fn parsed(output:Output)->serde_json::Value{assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));serde_json::from_slice(&output.stdout).unwrap()}

#[test]
fn cli_restart_allows_only_unreferenced_pending_missing_then_gc_can_finish(){
    let root=tempfile::tempdir().unwrap();let sources=tempfile::tempdir().unwrap();let s=open(root.path());let a=s.import_image(&image(sources.path(),"orphan.png",1)).unwrap();
    s.collect_garbage(ClipboardProtection::Unknown).unwrap();drop(s);std::fs::remove_file(&a.original).unwrap();
    let check=parsed(cli(root.path()));assert_eq!(check["schemaVersion"],2);assert_eq!(check["cases"],0);assert_eq!(check["assets"],1);assert_eq!(check["integrity"],"ok");assert_eq!(check["foreignKeys"],"ok");assert_eq!(check["caseAssetReferences"],0);assert!(check["sqliteVersion"].is_string());assert_eq!(check["originalHashes"],"ok_except_recoverable_pending_missing");assert_eq!(check["recoverablePendingMissing"],1);assert_eq!(check["recoverablePendingMissingIds"],serde_json::json!([a.id]));
    // The CLI exited and released the DB lease. A later GUI-style startup injects a reliable
    // empty clipboard and completes the pending metadata/cache cleanup, without system access.
    let s=open(root.path());let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.removed,1);assert!(report.failures.is_empty());drop(s);
    let clean=parsed(cli(root.path()));assert_eq!(clean["assets"],0);assert_eq!(clean["originalHashes"],"ok");assert_eq!(clean["recoverablePendingMissing"],0);
}

#[test]
fn cli_rejects_missing_original_for_live_trash_draft_or_unreferenced_live_row(){
    for kind in ["live","trash","draft","unreferenced_live"]{
        let root=tempfile::tempdir().unwrap();let sources=tempfile::tempdir().unwrap();let s=open(root.path());let a=s.import_image(&image(sources.path(),"required.png",2)).unwrap();
        match kind {
            "live"|"trash"=>{let mut d=s.edit_document(None).unwrap();d.title="case".into();d.prompt="prompt".into();d.references=vec![reference(&a.id)];s.commit_case(&d).unwrap();if kind=="trash"{s.soft_delete_case(&d.id).unwrap()}},
            "draft"=>{let sessions=EditSessions::default();let r=sessions.begin_persistent(&s,None,None,false,sessions.epoch()).unwrap();let mut d=r.document;d.references=vec![reference(&a.id)];sessions.stash(&s,&r.session_id,&d,1).unwrap();},
            _=>{}
        }
        drop(s);std::fs::remove_file(&a.original).unwrap();let output=cli(root.path());assert!(!output.status.success(),"{kind} incorrectly passed");assert!(String::from_utf8_lossy(&output.stderr).contains("stored_file_missing"),"{kind}: {}",String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn cli_rejects_corrupt_original_even_when_unreferenced_and_pending(){
    let root=tempfile::tempdir().unwrap();let sources=tempfile::tempdir().unwrap();let s=open(root.path());let a=s.import_image(&image(sources.path(),"pending.png",3)).unwrap();s.collect_garbage(ClipboardProtection::Unknown).unwrap();drop(s);std::fs::write(&a.original,b"corrupt").unwrap();let output=cli(root.path());assert!(!output.status.success());assert!(String::from_utf8_lossy(&output.stderr).contains("asset_hash_mismatch"));
}

#[test]
fn draft_asset_projection_ignores_unreferenced_pending_missing_but_check_reports_it(){
    let root=tempfile::tempdir().unwrap();let sources=tempfile::tempdir().unwrap();let s=open(root.path());let orphan=s.import_image(&image(sources.path(),"orphan.png",4)).unwrap();let required=s.import_image(&image(sources.path(),"draft.png",5)).unwrap();let sessions=EditSessions::default();let r=sessions.begin_persistent(&s,None,None,false,sessions.epoch()).unwrap();let draft_id=r.draft_id.unwrap();let mut d=r.document;d.references=vec![reference(&required.id)];sessions.stash(&s,&r.session_id,&d,1).unwrap();s.collect_garbage(ClipboardProtection::Unknown).unwrap();drop(s);std::fs::remove_file(&orphan.original).unwrap();
    let s=open(root.path());let report=s.collect_garbage(ClipboardProtection::Unknown).unwrap();assert_eq!(report.retained,1);
    let library=s.library().unwrap();assert_eq!(library.assets.len(),1);assert_eq!(library.assets[0].id,required.id);assert_eq!(s.draft_document(&draft_id).unwrap().0.references[0].asset_id,required.id);let check=s.check().unwrap();assert_eq!(check["assets"],2);assert_eq!(check["recoverablePendingMissing"],1);
    // Also validate recovery's real session path, beyond metadata projection alone.
    let sessions=EditSessions::default();let recovered=sessions.begin_persistent(&s,None,Some(&draft_id),false,sessions.epoch()).unwrap();assert_eq!(recovered.document.references[0].asset_id,required.id);drop(s);assert_eq!(parsed(cli(root.path()))["recoverablePendingMissing"],1);
}
