//! Headless-browser IPC bridge for integration tests. Never opens a default/user library.
use prompt_shelf_poc::{cases::{CaseDocument,EditSessions,ImageReference},paths::DataPaths,store::Store};
use serde_json::{json,Value};
use std::io::{BufRead,Write};
fn arg<'a>(r:&'a Value,key:&str)->Result<&'a str,String>{r["args"][key].as_str().ok_or(format!("missing {key}"))}
fn call(s:&Store,e:&EditSessions,r:&Value)->Result<Value,String>{Ok(match r["command"].as_str().unwrap_or(""){
    "get_library_index"=>serde_json::to_value(s.library_index()?).unwrap(),
    "get_case_assets"=>serde_json::to_value(s.assets_for_cases(&serde_json::from_value::<Vec<String>>(r["args"]["caseIds"].clone()).map_err(|e|e.to_string())?)?).unwrap(),
    "get_trash_index"=>serde_json::to_value(s.trash_index()?).unwrap(),
    "get_trash_assets"=>serde_json::to_value(s.trash_assets(&serde_json::from_value::<Vec<String>>(r["args"]["caseIds"].clone()).map_err(|e|e.to_string())?)?).unwrap(),
    "get_case_delete_draft"=>json!(s.case_delete_draft(arg(r,"caseId")?)?),
    "soft_delete_case"=>{e.while_idle(||s.soft_delete_case(arg(r,"caseId")?))?;Value::Null},
    "restore_case"=>{e.while_idle(||s.restore_case(arg(r,"caseId")?))?;Value::Null},
    "permanently_delete_case"=>{e.while_idle(||s.permanently_delete_case(arg(r,"caseId")?))?;Value::Null},
    "discard_case_draft"=>{e.discard_unopened(s,arg(r,"draftId")?)?;Value::Null},
    "test_suspend_editor"=>{e.clear();Value::Null},
    // Only this marked temp-directory bridge exposes anomaly injection; production has no such IPC.
    "test_create_related_draft"=>{let c=rusqlite::Connection::open(s.paths().database()?).map_err(|e|e.to_string())?;c.pragma_update(None,"foreign_keys",true).map_err(|e|e.to_string())?;let draft=uuid::Uuid::new_v4().to_string();let count=c.execute("INSERT INTO drafts(id,case_id,base_revision,title,prompt,project_id,updated_at,dirty) SELECT ?1,id,edit_revision,title,prompt||' related draft',project_id,1,?3 FROM cases WHERE id=?2",rusqlite::params![draft,arg(r,"caseId")?,r["args"]["dirty"].as_bool().unwrap_or(true)]).map_err(|e|e.to_string())?;if count!=1{return Err("test_case_missing".into())}json!(draft)},
    "catalog_create"=>{let id=e.while_idle(||s.catalog_create(arg(r,"kind")?,arg(r,"name")?))?;json!(id)},
    "catalog_rename"=>{e.while_idle(||s.catalog_rename(arg(r,"kind")?,arg(r,"id")?,arg(r,"name")?))?;Value::Null},
    "catalog_delete"=>{let ids=serde_json::from_value::<Vec<String>>(r["args"]["ids"].clone()).map_err(|e|e.to_string())?;e.while_idle(||s.catalog_delete(arg(r,"kind")?,&ids))?;Value::Null},
    "set_quick_tag"=>{e.while_idle(||s.set_quick_tag(arg(r,"id")?,r["args"]["enabled"].as_bool().unwrap()))?;Value::Null},
    "reorder_quick_tags"=>{let ids=serde_json::from_value::<Vec<String>>(r["args"]["ids"].clone()).map_err(|e|e.to_string())?;e.while_idle(||s.reorder_quick_tags(&ids))?;Value::Null},
    "set_favorite"=>{s.set_favorite(arg(r,"caseId")?,r["args"]["favorite"].as_bool().unwrap())?;Value::Null},
    "set_ui_preferences"=>{s.set_ui_preferences(r["args"]["patch"].clone())?;Value::Null},
    "save_dictionary"=>{s.save_dictionary(serde_json::from_value::<Vec<Vec<String>>>(r["args"]["groups"].clone()).map_err(|e|e.to_string())?)?;Value::Null},
    "test_record_use"=>{s.record_use(arg(r,"caseId")?)?;Value::Null},
    "begin_case_edit"=>serde_json::to_value(e.begin_persistent(s,r["args"]["caseId"].as_str(),None,false,e.epoch())?).unwrap(),
    "save_case_draft"=>json!(e.stash(s,arg(r,"sessionId")?,&serde_json::from_value::<CaseDocument>(r["args"]["document"].clone()).map_err(|e|e.to_string())?,r["args"]["saveSeq"].as_u64().ok_or("seq")?)?),
    "commit_case"=>serde_json::to_value(e.save(s,arg(r,"sessionId")?,&serde_json::from_value::<CaseDocument>(r["args"]["document"].clone()).map_err(|e|e.to_string())?)?).unwrap(),
    "cancel_case_edit"=>{e.discard(s,arg(r,"sessionId")?)?;Value::Null},
    "recover_case_draft"=>serde_json::to_value(e.begin_persistent(s,None,Some(arg(r,"draftId")?),r["args"]["asNew"].as_bool().unwrap_or(false),e.epoch())?).unwrap(),
    "list_case_drafts"=>serde_json::to_value(s.draft_summaries()?).unwrap(),
    "get_library"=>serde_json::to_value(s.library()?).unwrap(),
    "normalize_source_url"=>json!(prompt_shelf_poc::cases::normalize_source(arg(r,"value")?)?),
    "import_case_images"=>{e.check(arg(r,"sessionId")?)?;let files=r["args"]["paths"].as_array().cloned().unwrap_or_else(||vec![json!(s.paths().root().join("test-import.png").to_string_lossy())]);let mut output=vec![];for file in files{let path=std::path::Path::new(file.as_str().ok_or("path")?);let asset=s.import_image(path)?;let mut value=serde_json::to_value(asset).unwrap();value["originalName"]=json!(path.file_name().unwrap().to_string_lossy());output.push(value)}e.check(arg(r,"sessionId")?)?;json!(output)},
    "probe"=>{let drafts=s.draft_summaries()?;let documents=drafts.iter().map(|d|{let (document,seq)=s.draft_document(&d.id).unwrap();json!({"draftId":d.id,"document":document,"seq":seq})}).collect::<Vec<_>>();json!({"library":s.library()?,"drafts":drafts,"documents":documents})},
    _=>return Err("unsupported_test_command".into())
})}
#[test]
fn browser_store_bridge(){
    let Some(root)=std::env::var_os("PROMPT_SHELF_BROWSER_TEST_DIR") else{return};let root=std::path::PathBuf::from(root);
    assert!(root.join("browser-test-owned").is_file(),"Only a test-owned directory may be opened");
    let store=Store::open(DataPaths::new(&root).unwrap()).unwrap();
    if !root.join("browser-test-seeded").is_file(){
        let mut assets=vec![];
        for (index,(width,height)) in [(12,8),(8,12),(10,10),(13,10)].into_iter().enumerate(){let file=root.join(format!("test-{index}.png"));image::RgbaImage::from_pixel(width,height,image::Rgba([40+index as u8*20,90,120,160])).save(&file).unwrap();assets.push(store.import_image(&file).unwrap().id)}
        image::RgbaImage::from_pixel(15,10,image::Rgba([160,80,40,120])).save(root.join("test-import.png")).unwrap();
        let mut d=store.edit_document(None).unwrap();d.title="正式 A".into();d.prompt="正式提示词 A".into();d.result=Some(ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:assets[0].clone(),original_name:"效果.png".into()});d.references=assets.iter().enumerate().map(|(i,asset)|ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:asset.clone(),original_name:format!("参考{i}.png")}).collect();store.commit_case(&d).unwrap();
        let mut b=store.edit_document(None).unwrap();b.title="共享案例 B".into();b.prompt="正式 B".into();b.references=vec![ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:assets[0].clone(),original_name:"共享.png".into()}];store.commit_case(&b).unwrap();
        std::fs::write(root.join("browser-test-seeded"),"synthetic fixture seeded once").unwrap();
    }
    let edits=EditSessions::default();println!("BRIDGE {}",json!({"ready":true}));std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines(){let line=line.unwrap();let request:Value=serde_json::from_str(&line).unwrap();let result=call(&store,&edits,&request);let reply=match result{Ok(value)=>json!({"id":request["id"],"value":value}),Err(error)=>json!({"id":request["id"],"error":error})};println!("BRIDGE {reply}");std::io::stdout().flush().unwrap();}
}
