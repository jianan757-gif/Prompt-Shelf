use super::*;
use crate::{cases::{CaseDocument, EditSessions, ImageReference}, paths::DataPaths};
use std::{path::Path, sync::{Arc, Barrier}};

fn store(path: &Path) -> Store { Store::open(DataPaths::new(path).unwrap()).unwrap() }
fn image(root: &Path, name: &str, color: u8) -> PathBuf {
    let path=root.join(name);
    image::RgbaImage::from_pixel(12,8,image::Rgba([color,80,120,96])).save(&path).unwrap();
    path
}
fn reference(id: &str) -> ImageReference { ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:id.into(),original_name:"image.png".into()} }
fn case(s:&Store, asset: &str) -> CaseDocument {
    let mut d=s.edit_document(None).unwrap();d.title="case".into();d.prompt="prompt".into();d.references=vec![reference(asset)];
    s.commit_case(&d).unwrap();s.edit_document(Some(&d.id)).unwrap()
}
fn asset_count(s:&Store)->i64{s.connection.lock().unwrap().query_row("SELECT count(*) FROM assets",[],|r|r.get(0)).unwrap()}
fn remove_case_relations(s:&Store,id:&str){
    let _resources=s.resources.lock().unwrap();let c=s.connection.lock().unwrap();
    c.execute("DELETE FROM cases WHERE id=?1",[id]).unwrap();mark_unreferenced(&c).unwrap();
}

#[test]
fn imported_asset_is_pinned_through_unattached_and_discarded_draft_then_collected_on_reopen(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let path=image(source.path(),"a.png",1);
    let s=store(root.path());let a=s.import_image(&path).unwrap();
    let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.retained,1);assert!(Path::new(&a.original).exists());
    let sessions=EditSessions::default();let r=sessions.begin_persistent(&s,None,None,false,sessions.epoch()).unwrap();
    let mut d=r.document;d.references=vec![reference(&a.id)];sessions.stash(&s,&r.session_id,&d,1).unwrap();
    sessions.discard(&s,&r.session_id).unwrap();
    assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap().retained,1);
    drop(s);let s=store(root.path());let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();
    assert_eq!(report.removed,1);assert!(report.failures.is_empty());assert_eq!(asset_count(&s),0);assert!(!Path::new(&a.original).exists());
}

#[test]
fn live_trash_and_draft_relations_protect_without_import_pins(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());
    let live=s.import_image(&image(source.path(),"live.png",2)).unwrap();let trash=s.import_image(&image(source.path(),"trash.png",3)).unwrap();let draft=s.import_image(&image(source.path(),"draft.png",4)).unwrap();
    let a=case(&s,&live.id);let b=case(&s,&trash.id);s.connection.lock().unwrap().execute("UPDATE cases SET deleted_at=100 WHERE id=?1",[&b.id]).unwrap();
    let sessions=EditSessions::default();let r=sessions.begin_persistent(&s,None,None,false,sessions.epoch()).unwrap();let mut d=r.document;d.references=vec![reference(&draft.id)];sessions.stash(&s,&r.session_id,&d,1).unwrap();
    drop(s);let s=store(root.path());let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.candidates,0);assert_eq!(asset_count(&s),3);
    assert!(s.edit_document(Some(&a.id)).is_ok());assert!(Path::new(&trash.original).exists());assert!(Path::new(&draft.original).exists());
}

#[test]
fn stale_pending_state_and_missing_file_never_override_a_current_draft_reference(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());let a=s.import_image(&image(source.path(),"a.png",42)).unwrap();
    let sessions=EditSessions::default();let r=sessions.begin_persistent(&s,None,None,false,sessions.epoch()).unwrap();let mut d=r.document;d.references=vec![reference(&a.id)];sessions.stash(&s,&r.session_id,&d,1).unwrap();drop(s);
    let s=store(root.path());s.connection.lock().unwrap().execute("UPDATE assets SET gc_state='pending' WHERE id=?1",[&a.id]).unwrap();std::fs::remove_file(&a.original).unwrap();
    let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.candidates,0);assert_eq!(asset_count(&s),1);
    assert!(s.check().is_err(),"A missing draft original must fail the offline integrity gate");
    assert_eq!(s.connection.lock().unwrap().query_row("SELECT gc_state FROM assets WHERE id=?1",[&a.id],|r|r.get::<_,String>(0)).unwrap(),"live");
}

#[test]
fn output_snapshot_keeps_original_after_case_delete_and_queue_cancellation_until_reopen(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());let a=s.import_image(&image(source.path(),"a.png",5)).unwrap();let d=case(&s,&a.id);drop(s);
    let s=store(root.path());let snapshot=s.output_snapshot_at_revision(&d.id,d.base_revision).unwrap();assert_eq!(snapshot.assets.len(),1);
    remove_case_relations(&s,&d.id);
    // Output pins deliberately have no queue-complete or cancellation release operation.
    assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap().retained,1);assert!(Path::new(&a.original).exists());
    assert!(s.output_snapshot_at_revision(&d.id,d.base_revision).is_err());drop(s);
    let s=store(root.path());assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap().removed,1);
}

#[test]
fn clipboard_alias_unknown_and_unreadable_paths_conservatively_retain(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());let a=s.import_image(&image(source.path(),"a.png",6)).unwrap();drop(s);let s=store(root.path());
    assert_eq!(s.collect_garbage(ClipboardProtection::Unknown).unwrap().retained,1);
    assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![PathBuf::from(&a.original)])).unwrap().retained,1);
    assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![source.path().join("missing")])).unwrap().retained,1);
    assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap().removed,1);
}

#[test]
fn missing_candidate_file_is_rebuilt_by_same_hash_import_and_returns_live_reference(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let path=image(source.path(),"a.png",7);let s=store(root.path());let a=s.import_image(&path).unwrap();
    {let c=s.connection.lock().unwrap();mark_unreferenced(&c).unwrap();}
    std::fs::remove_file(&a.original).unwrap();
    let rebuilt=s.import_image(&path).unwrap();assert_eq!(a.id,rebuilt.id);assert_eq!(a.sha256,rebuilt.sha256);assert!(Path::new(&rebuilt.original).exists());
    assert_eq!(s.connection.lock().unwrap().query_row("SELECT gc_state FROM assets WHERE id=?1",[&a.id],|r|r.get::<_,String>(0)).unwrap(),"live");
    let d=case(&s,&rebuilt.id);assert_eq!(s.edit_document(Some(&d.id)).unwrap().references[0].asset_id,rebuilt.id);
}

#[test]
fn cleanup_failure_retains_pending_row_and_retries_after_partial_file_removal(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());let a=s.import_image(&image(source.path(),"a.png",8)).unwrap();drop(s);let s=store(root.path());
    s.connection.lock().unwrap().execute_batch("CREATE TRIGGER test_gc_fail BEFORE DELETE ON assets BEGIN SELECT RAISE(ABORT,'injected_db_delete_failure'); END;").unwrap();
    let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.failures.len(),1);assert!(report.failures[0].error.contains("injected_db_delete_failure"));assert!(!Path::new(&a.original).exists());assert_eq!(asset_count(&s),1);
    s.connection.lock().unwrap().execute_batch("DROP TRIGGER test_gc_fail").unwrap();
    let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.removed,1);assert!(report.failures.is_empty());assert_eq!(asset_count(&s),0);
}

#[cfg(windows)]
#[test]
fn windows_open_original_retains_pending_on_unlink_failure_then_retries(){
    use std::os::windows::fs::OpenOptionsExt;
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());let a=s.import_image(&image(source.path(),"a.png",43)).unwrap();drop(s);let s=store(root.path());
    let held=std::fs::OpenOptions::new().read(true).share_mode(0).open(&a.original).unwrap();
    let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.failures.len(),1);assert!(report.failures[0].error.contains("gc_remove_failed"));assert!(Path::new(&a.original).exists());assert_eq!(asset_count(&s),1);
    assert_eq!(s.connection.lock().unwrap().query_row("SELECT gc_state FROM assets WHERE id=?1",[&a.id],|r|r.get::<_,String>(0)).unwrap(),"pending");
    drop(held);let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.removed,1);assert!(report.failures.is_empty());assert_eq!(asset_count(&s),0);
}

#[test]
fn importer_and_gc_serialize_across_file_prepare_registration_and_late_case_attachment(){
    for n in 0..8 {
        let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let path=image(source.path(),"a.png",20+n);let s=store(root.path());s.import_image(&path).unwrap();drop(s);
        let s=Arc::new(store(root.path()));let barrier=Arc::new(Barrier::new(3));
        let a={let s=s.clone();let barrier=barrier.clone();let path=path.clone();std::thread::spawn(move||{barrier.wait();s.import_image(&path).unwrap()})};
        let gc={let s=s.clone();let barrier=barrier.clone();std::thread::spawn(move||{barrier.wait();s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap()})};
        barrier.wait();let imported=a.join().unwrap();let report=gc.join().unwrap();assert!(report.failures.is_empty());assert!(Path::new(&imported.original).exists());
        let d=case(&s,&imported.id);assert_eq!(s.edit_document(Some(&d.id)).unwrap().references[0].asset_id,imported.id);
        assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap().removed,0);
    }
}

#[test]
fn manifest_prepare_and_gc_share_the_same_resource_coordinator(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();image(source.path(),"a.png",40);image(source.path(),"thumb.png",40);
    let manifest=source.path().join("manifest.json");std::fs::write(&manifest,serde_json::to_vec(&serde_json::json!({"schema_version":1,"assets":[{"id":"a","original":"a.png","thumb":"thumb.png","width":1,"height":1}],"cases":[{"id":"c","title":"one","prompt":"p","project":"","tags":[],"favorite":false,"used":0,"result":"a","refs":[]}]})).unwrap()).unwrap();
    let s=Arc::new(store(root.path()));let barrier=Arc::new(Barrier::new(3));let import={let s=s.clone();let barrier=barrier.clone();std::thread::spawn(move||{barrier.wait();s.import_manifest(&manifest)})};let gc={let s=s.clone();let barrier=barrier.clone();std::thread::spawn(move||{barrier.wait();s.collect_garbage(ClipboardProtection::Files(vec![]))})};barrier.wait();import.join().unwrap().unwrap();assert!(gc.join().unwrap().unwrap().failures.is_empty());assert_eq!(s.counts().unwrap(),(1,1));assert_eq!(s.library().unwrap().assets.len(),1);
}

#[test]
fn gc_rejects_managed_directory_replaced_by_link_when_supported(){
    let root=tempfile::tempdir().unwrap();let outside=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let s=store(root.path());let a=s.import_image(&image(source.path(),"a.png",41)).unwrap();drop(s);let s=store(root.path());
    let assets=root.path().join("assets");let saved=root.path().join("saved-assets");std::fs::rename(&assets,&saved).unwrap();
    #[cfg(windows)] let link=std::os::windows::fs::symlink_dir(outside.path(),&assets).or_else(|error| {
        if error.raw_os_error()!=Some(1314) { return Err(error); }
        // Only creates a junction between our two tempfile roots; no shell deletion/move.
        let result=std::process::Command::new("cmd").args(["/c","mklink","/J"]).arg(&assets).arg(outside.path()).output()?;
        if result.status.success() { eprintln!("JUNCTION_VERIFIED: testing an unprivileged temp junction");Ok(()) }
        else { Err(error) }
    });
    #[cfg(unix)] let link=std::os::unix::fs::symlink(outside.path(),&assets);
    if let Err(e)=link {
        #[cfg(windows)] if e.raw_os_error()==Some(1314){std::fs::rename(&saved,&assets).unwrap();eprintln!("SYMLINK_UNAVAILABLE: no Windows privilege; linked-directory branch not exercised");return;}
        panic!("link setup failed: {e}");
    }
    let outside_file=outside.path().join(Path::new(&a.original).file_name().unwrap());std::fs::copy(saved.join(outside_file.file_name().unwrap()),&outside_file).unwrap();
    let report=s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap();assert_eq!(report.failures.len(),1);assert!(outside_file.exists());assert_eq!(asset_count(&s),1);assert!(s.import_image(&source.path().join("a.png")).is_err());
    #[cfg(windows)] std::fs::remove_dir(&assets).unwrap();
    #[cfg(unix)] std::fs::remove_file(&assets).unwrap();
    std::fs::rename(&saved,&assets).unwrap();assert_eq!(s.collect_garbage(ClipboardProtection::Files(vec![])).unwrap().removed,1);assert!(outside_file.exists());
}
