//! Existing queue + production SQLite usage accounting; no native clipboard/input.
use super::*;
use std::sync::atomic::{AtomicUsize,AtomicBool};
struct Mock { fail_prepare:bool,fail_dispatch:bool,sent:AtomicUsize }
impl PastePlatform for Mock{
 type Target=();type Prepared=PasteItem;
 fn capture_target(&self)->Result<(),String>{Ok(())}
 fn target_is_foreground(&self,_:&())->Result<(),String>{Ok(())}
 fn request_restore(&self,_:&())->Result<(),String>{Ok(())}
 fn restoration_status(&self,_:&())->Result<bool,String>{Ok(true)}
 fn may_begin(&self,_:&())->Result<(),String>{Ok(())}
 fn prepare(&self,item:PasteItem)->Result<PasteItem,String>{if self.fail_prepare{Err("prepare_fail".into())}else{Ok(item)}}
 fn write_clipboard(&self,_:PasteItem)->Result<(),String>{Ok(())}
 fn dispatch(&self,_:&())->Result<(),String>{if self.fail_dispatch{Err("dispatch_fail".into())}else{self.sent.fetch_add(1,Ordering::SeqCst);Ok(())}}
}
#[tokio::test]
async fn real_queue_counts_only_first_success_even_after_cancel_and_never_replays_on_record_error(){
 for mode in ["prepare-fail","dispatch-fail","partial-cancel","completed","record-fail"]{
  let root=tempfile::tempdir().unwrap();let store=Store::open(DataPaths::new(root.path()).unwrap()).unwrap();let mut document=store.edit_document(None).unwrap();document.title="usage".into();document.prompt="original".into();store.commit_case(&document).unwrap();
  if mode=="record-fail"{let c=rusqlite::Connection::open(store.paths().database().unwrap()).unwrap();c.execute_batch("CREATE TRIGGER fail_use BEFORE UPDATE OF use_count ON cases BEGIN SELECT RAISE(ABORT,'injected_usage'); END;").unwrap();}
  let platform=Arc::new(Mock{fail_prepare:mode=="prepare-fail",fail_dispatch:mode=="dispatch-fail",sent:AtomicUsize::new(0)});let queue=queue::Queue::default();let ticket=queue.reserve().unwrap();let id=ticket.id;let once=prompt_shelf_poc::usage::FirstDispatch::default();let failures=AtomicUsize::new(0);let was_cancelled=AtomicBool::new(false);
  queue::run(platform.clone(),(),ticket,document.id.clone(),vec![PasteItem::Text("one".into()),PasteItem::Text("two".into())],Duration::from_millis(1),|event|{
   if let Some(result)=once.observe(event.done,||store.record_use(&event.case_id)){if result.is_err(){failures.fetch_add(1,Ordering::SeqCst);}}
   if mode=="partial-cancel"&&event.done==1&&!was_cancelled.swap(true,Ordering::SeqCst){queue.cancel_request(id);}
  }).await;
  let case=store.library().unwrap().cases.remove(0);
  assert_eq!(case.used,if ["partial-cancel","completed"].contains(&mode){1}else{0},"{mode}");assert_eq!(case.edit_revision,1);assert_eq!(case.prompt,"original");
  assert_eq!(platform.sent.load(Ordering::SeqCst),match mode{"prepare-fail"|"dispatch-fail"=>0,"partial-cancel"=>1,_=>2},"{mode}");assert_eq!(failures.load(Ordering::SeqCst),usize::from(mode=="record-fail"));
 }
}
#[test]
fn catalog_gate_rejects_active_editor_and_late_writes_after_success_cannot_revive_names(){
 use prompt_shelf_poc::cases::EditSessions;
 let root=tempfile::tempdir().unwrap();let store=Store::open(DataPaths::new(root.path()).unwrap()).unwrap();let project=store.catalog_create("projects","Old").unwrap();let mut d=store.edit_document(None).unwrap();d.title="A".into();d.prompt="unchanged".into();d.project="Old".into();store.commit_case(&d).unwrap();
 let edits=EditSessions::default();let receipt=edits.begin_persistent(&store,Some(&d.id),None,false,edits.epoch()).unwrap();let old_epoch=edits.epoch();assert!(edits.while_idle(||store.catalog_rename("projects",&project,"New")).is_err());assert_eq!(store.library().unwrap().projects[0].name,"Old");
 edits.stash(&store,&receipt.session_id,&receipt.document,1).unwrap();edits.discard(&store,&receipt.session_id).unwrap();edits.while_idle(||store.catalog_rename("projects",&project,"New")).unwrap();assert!(edits.stash(&store,&receipt.session_id,&receipt.document,99).is_err());assert!(edits.begin_persistent(&store,None,None,false,old_epoch).is_err());let library=store.library().unwrap();assert_eq!(library.projects.len(),1);assert_eq!(library.projects[0].name,"New");assert_eq!(library.cases[0].prompt,"unchanged");assert_eq!(library.cases[0].project_id.as_deref(),Some(project.as_str()));
}
