//! Real E CLI subprocesses use the same root-level pointer and lease as the GUI.
use prompt_shelf_poc::{backup::{prepare_restore,JobControl},storage_host::LibraryHost,paths::DataPaths,store::Store};
fn add(store:&Store,title:&str){let mut d=store.edit_document(None).unwrap();d.title=title.into();d.prompt="正文\nsecond line".into();store.commit_case(&d).unwrap();}
#[test]fn cli_resolves_active_storage_and_cannot_bypass_root_lease(){
    let root=tempfile::tempdir().unwrap();let source=tempfile::tempdir().unwrap();let output=tempfile::tempdir().unwrap();
    let incoming=Store::open(DataPaths::new(source.path()).unwrap()).unwrap();add(&incoming,"new A");add(&incoming,"new B");let zip=output.path().join("incoming.zip");incoming.export_backup(&zip,&JobControl::new("export"),|_|{}).unwrap();drop(incoming);
    let host=LibraryHost::open(root.path()).unwrap();host.write(1,|s|{add(s,"old");Ok(())}).unwrap();let ready=prepare_restore(&zip,&host.storage_parent().unwrap(),&JobControl::new("inspect"),|_|{}).unwrap().into_storage().unwrap();let revision=host.revision();let mut guard=host.begin_maintenance(1).unwrap();guard.install(&ready,revision).unwrap();drop(guard);
    let check=||std::process::Command::new(env!("CARGO_BIN_EXE_prompt-shelf-store")).arg("--data-dir").arg(root.path()).arg("--check").output().unwrap();
    let locked=check();assert!(!locked.status.success());assert!(String::from_utf8_lossy(&locked.stderr).contains("library_in_use"));drop(host);
    let result=check();assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));let value:serde_json::Value=serde_json::from_slice(&result.stdout).unwrap();assert_eq!(value["cases"],2);assert_eq!(value["originalHashes"],"ok");
    // Opening the retained flat Store directly confirms it was not overwritten by the restore.
    let old=Store::open(DataPaths::new(root.path()).unwrap()).unwrap();assert_eq!(old.library_index().unwrap().cases[0].title,"old");
}
