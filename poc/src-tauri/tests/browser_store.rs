//! Headless-browser IPC bridge for integration tests. Never opens a default/user library.
use prompt_shelf_poc::{cases::{CaseDocument,EditSessions,ImageReference},paths::DataPaths,store::Store};
use serde_json::{json,Value};
use std::io::{BufRead,Write};
fn arg<'a>(r:&'a Value,key:&str)->Result<&'a str,String>{r["args"][key].as_str().ok_or(format!("missing {key}"))}
fn call(s:&Store,e:&EditSessions,r:&Value)->Result<Value,String>{Ok(match r["command"].as_str().unwrap_or(""){
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
    if store.counts().unwrap().0==0{
        let mut assets=vec![];
        for (index,(width,height)) in [(12,8),(8,12),(10,10),(13,10)].into_iter().enumerate(){let file=root.join(format!("test-{index}.png"));image::RgbaImage::from_pixel(width,height,image::Rgba([40+index as u8*20,90,120,160])).save(&file).unwrap();assets.push(store.import_image(&file).unwrap().id)}
        image::RgbaImage::from_pixel(15,10,image::Rgba([160,80,40,120])).save(root.join("test-import.png")).unwrap();
        let mut d=store.edit_document(None).unwrap();d.title="正式 A".into();d.prompt="正式提示词 A".into();d.result=Some(ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:assets[0].clone(),original_name:"效果.png".into()});d.references=assets.iter().enumerate().map(|(i,asset)|ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:asset.clone(),original_name:format!("参考{i}.png")}).collect();store.commit_case(&d).unwrap();
        let mut b=store.edit_document(None).unwrap();b.title="共享案例 B".into();b.prompt="正式 B".into();b.references=vec![ImageReference{id:uuid::Uuid::new_v4().to_string(),asset_id:assets[0].clone(),original_name:"共享.png".into()}];store.commit_case(&b).unwrap();
    }
    let edits=EditSessions::default();println!("BRIDGE {}",json!({"ready":true}));std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines(){let line=line.unwrap();let request:Value=serde_json::from_str(&line).unwrap();let result=call(&store,&edits,&request);let reply=match result{Ok(value)=>json!({"id":request["id"],"value":value}),Err(error)=>json!({"id":request["id"],"error":error})};println!("BRIDGE {reply}");std::io::stdout().flush().unwrap();}
}
