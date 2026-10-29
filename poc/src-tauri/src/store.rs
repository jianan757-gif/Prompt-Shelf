use crate::{assets, data, paths::DataPaths};
use rusqlite::{params, Connection};
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    path::Path,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: i64 = 2;
pub(crate) type AssetRow=(String,String,String,u32,u32);
const INITIAL: &str = include_str!("migrations/001_initial.sql");
pub struct Store {
    // Lock order: edit session -> resources -> database. Never acquire resources under DB.
    pub(crate) resources: Mutex<crate::gc::ResourceLifecycle>,
    pub(crate) connection: Mutex<Connection>,
    paths: DataPaths,
    // Held for this Store's whole lifetime, including offline CLI. Never unlink a live lock file.
    _lease: File,
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn sql(error: rusqlite::Error) -> String {
    format!("sqlite: {error}")
}
fn apply_initial(connection: &mut Connection, script: &str) -> Result<(), String> {
    let transaction = connection.transaction().map_err(sql)?;
    transaction.execute_batch(script).map_err(sql)?;
    transaction
        .pragma_update(None, "user_version", 1)
        .map_err(sql)?;
    transaction.commit().map_err(sql)
}
impl Store {
    pub fn open(paths: DataPaths) -> Result<Self, String> {
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(paths.lock()?)
            .map_err(|e| e.to_string())?;
        lease
            .try_lock()
            .map_err(|e| format!("library_in_use: {e}"))?;
        let mut connection = Connection::open(paths.database()?).map_err(sql)?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(sql)?;
        // Reject before any persistent pragma or migration. Never reset an unknown database.
        if version > SCHEMA_VERSION {
            return Err(format!("future_schema: {version}"));
        }
        connection
            .busy_timeout(Duration::from_secs(3))
            .map_err(sql)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(sql)?;
        if version == 0 {
            let tables:i64=connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(sql)?;
            if tables != 0 {
                return Err("unversioned_nonempty_database".into());
            }
            apply_initial(&mut connection, INITIAL)?;
        }
        if version < 2 {
            let tx = connection.transaction().map_err(sql)?;
            tx.execute_batch(include_str!("migrations/002_drafts.sql")).map_err(sql)?;
            tx.pragma_update(None, "user_version", 2).map_err(sql)?;
            tx.commit().map_err(sql)?;
        }
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(sql)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(sql)?;
        Ok(Self {
            resources: Mutex::new(Default::default()),
            connection: Mutex::new(connection),
            paths,
            _lease: lease,
        })
    }
    pub fn paths(&self) -> &DataPaths {
        &self.paths
    }
    /// Incremental asset service for the next editing stage; no case/draft mutation here.
    pub fn import_image(&self, source: &Path) -> Result<data::Asset, String> {
        let mut resources = self.resources.lock().map_err(|e| e.to_string())?;
        self.paths.validate_managed_area("assets")?;
        self.paths.validate_managed_area("cache/thumbs")?;
        self.paths.validate_managed_area("staging")?;
        let ready = assets::prepare(&self.paths, source, "")?;
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        connection.execute("INSERT INTO assets(id,sha256,extension,mime,byte_size,width,height,relative_path,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(sha256) DO NOTHING",params![id(),ready.hash,ready.extension,ready.mime,ready.bytes as i64,ready.width,ready.height,ready.relative,now()]).map_err(sql)?;
        let asset_id: String = connection
            .query_row(
                "SELECT id FROM assets WHERE sha256=?1",
                [&ready.hash],
                |r| r.get(0),
            )
            .map_err(sql)?;
        connection.execute("UPDATE assets SET gc_state='live' WHERE id=?1", [&asset_id]).map_err(sql)?;
        // Imported files can wait in the renderer before draft_assets catches up.
        // Pin conservatively for this run, including a cancelled/failed import result.
        resources.imported.insert(asset_id.clone());
        drop(connection);
        Ok(data::Asset {
            id: asset_id,
            sha256: ready.hash.clone(),
            original: self
                .paths
                .resolve(&ready.relative, "assets")?
                .to_string_lossy()
                .into_owned(),
            thumbnail: self
                .paths
                .resolve(&DataPaths::thumb_relative(&ready.hash)?, "cache/thumbs")?
                .to_string_lossy()
                .into_owned(),
            width: ready.width,
            height: ready.height,
        })
    }
    pub fn thumbnail_files(
        &self,
        library: &data::Library,
    ) -> Result<Vec<std::path::PathBuf>, String> {
        library
            .assets
            .iter()
            .map(|asset| {
                let expected = self
                    .paths
                    .resolve(&DataPaths::thumb_relative(&asset.sha256)?, "cache/thumbs")?;
                let actual = dunce::canonicalize(&asset.thumbnail).map_err(|e| e.to_string())?;
                if expected != actual {
                    return Err("thumbnail_projection_mismatch".into());
                }
                Ok(expected)
            })
            .collect()
    }
    fn empty(connection: &Connection) -> Result<bool, String> {
        connection.query_row("SELECT (SELECT count(*) FROM cases)+(SELECT count(*) FROM assets)+(SELECT count(*) FROM projects)+(SELECT count(*) FROM tags)+(SELECT count(*) FROM drafts)+(SELECT count(*) FROM settings)+(SELECT count(*) FROM synonym_groups)",[],|r|r.get::<_,i64>(0)).map(|n|n==0).map_err(sql)
    }
    /// Development-only batch seed. Ready files precede one all-or-nothing metadata transaction.
    pub fn import_manifest(&self, path: &Path) -> Result<(), String> {
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        self.paths.validate_managed_area("assets")?;
        self.paths.validate_managed_area("cache/thumbs")?;
        self.paths.validate_managed_area("staging")?;
        if !Self::empty(&*self.connection.lock().map_err(|e| e.to_string())?)? {
            return Err("manifest_import_requires_empty_library".into());
        }
        let source = data::Library::load(path)?;
        let mut prepared = HashMap::new();
        let mut aliases = HashMap::new();
        let mut names = HashMap::new();
        for asset in &source.assets {
            let ready = assets::prepare(&self.paths, Path::new(&asset.original), &asset.sha256)?;
            aliases.insert(asset.id.clone(), ready.hash.clone());
            names.insert(
                asset.id.clone(),
                Path::new(&asset.original)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            );
            prepared.entry(ready.hash.clone()).or_insert(ready);
        }
        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql)?;
        if !Self::empty(&tx)? {
            return Err("manifest_import_requires_empty_library".into());
        }
        let timestamp = now();
        let mut asset_ids = HashMap::new();
        for ready in prepared.values() {
            let asset_id = id();
            tx.execute("INSERT INTO assets(id,sha256,extension,mime,byte_size,width,height,relative_path,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![asset_id,ready.hash,ready.extension,ready.mime,ready.bytes as i64,ready.width,ready.height,ready.relative,timestamp]).map_err(sql)?;
            asset_ids.insert(ready.hash.clone(), asset_id);
        }
        let mut projects = HashMap::new();
        let mut tags = HashMap::new();
        for case in &source.cases {
            let project = case.project.trim();
            let project_id = if project.is_empty() {
                None
            } else {
                Some(catalog(&tx, "projects", project, &mut projects, timestamp)?)
            };
            let case_id = id();
            let used = i64::try_from(case.used).map_err(|_| "use_count_overflow")?;
            tx.execute("INSERT INTO cases(id,title,prompt,project_id,favorite,created_at,updated_at,last_used_at,use_count) VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?8)",params![case_id,case.title,case.prompt,project_id,case.favorite,timestamp,if used>0{Some(timestamp)}else{None},used]).map_err(sql)?;
            let mut seen = HashSet::new();
            let mut position = 0;
            for tag in &case.tags {
                let key = tag.trim().to_lowercase();
                if !seen.insert(key) {
                    continue;
                }
                let tag_id = catalog(&tx, "tags", tag.trim(), &mut tags, timestamp)?;
                tx.execute(
                    "INSERT INTO case_tags(case_id,tag_id,position) VALUES(?1,?2,?3)",
                    params![case_id, tag_id, position],
                )
                .map_err(sql)?;
                position += 1;
            }
            for (role, values) in [
                ("result", case.result.iter().collect::<Vec<_>>()),
                ("reference", case.refs.iter().collect()),
            ] {
                for (position, alias) in values.into_iter().enumerate() {
                    let asset_id = &asset_ids[&aliases[alias]];
                    tx.execute("INSERT INTO case_assets(id,case_id,asset_id,role,position,original_name) VALUES(?1,?2,?3,?4,?5,?6)",params![id(),case_id,asset_id,role,position as i64,names[alias]]).map_err(sql)?;
                }
            }
        }
        tx.commit().map_err(sql)
    }
    pub fn counts(&self) -> Result<(usize, usize), String> {
        let c = self.connection.lock().map_err(|e| e.to_string())?;
        c.query_row("SELECT (SELECT count(*) FROM cases WHERE deleted_at IS NULL),(SELECT count(*) FROM assets)",[],|r|Ok((r.get::<_,i64>(0)? as usize,r.get::<_,i64>(1)? as usize))).map_err(sql)
    }
    pub fn library(&self) -> Result<data::Library, String> {
        self.project(None, true)
    }
    pub fn library_index(&self) -> Result<data::Library,String> { self.project_inner(None,false,false) }
    pub fn case_snapshot(&self, case_id: &str) -> Result<data::Library, String> {
        self.project(Some(case_id), false)
    }
    fn project(&self, case_id: Option<&str>, thumbnails: bool) -> Result<data::Library, String> { self.project_inner(case_id,thumbnails,true) }
    fn project_inner(&self, case_id: Option<&str>, thumbnails: bool, include_assets:bool) -> Result<data::Library, String> {
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let mut statement=connection.prepare("SELECT c.id,c.title,c.prompt,coalesce(p.name,''),c.favorite,c.use_count,c.source_url,c.created_at,c.updated_at,c.last_used_at,c.edit_revision,c.project_id FROM cases c LEFT JOIN projects p ON p.id=c.project_id WHERE c.deleted_at IS NULL AND (?1 IS NULL OR c.id=?1) ORDER BY c.created_at DESC,c.id").map_err(sql)?;
        let mut cases = statement
            .query_map([case_id], |r| {
                Ok(data::Case {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    prompt: r.get(2)?,
                    project: r.get(3)?,
                    favorite: r.get(4)?,
                    used: r.get::<_, i64>(5)? as u64,
                    source_url:r.get(6)?,created_at:r.get(7)?,updated_at:r.get(8)?,last_used_at:r.get(9)?,edit_revision:r.get::<_,i64>(10)? as u64,
                    project_id:r.get(11)?,tag_ids:vec![],
                    tags: vec![],
                    result: None,
                    refs: vec![],
                })
            })
            .map_err(sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql)?;
        drop(statement);
        for case in &mut cases {
            let mut tags=connection.prepare("SELECT t.name,t.id FROM case_tags ct JOIN tags t ON t.id=ct.tag_id WHERE ct.case_id=?1 ORDER BY ct.position").map_err(sql)?;
            let pairs=tags.query_map([&case.id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
            case.tags=pairs.iter().map(|(name,_)|name.clone()).collect();
            case.tag_ids=pairs.into_iter().map(|(_,id)|id).collect();
            let mut images = connection
                .prepare(
                    "SELECT asset_id,role FROM case_assets WHERE case_id=?1 ORDER BY role,position",
                )
                .map_err(sql)?;
            for image in images
                .query_map([&case.id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(sql)?
            {
                let (asset, role) = image.map_err(sql)?;
                if role == "result" {
                    case.result = Some(asset)
                } else {
                    case.refs.push(asset)
                }
            }
        }
        let rows = if include_assets {
            // Full editor recovery needs every referenced draft/trash asset, but no orphan rows.
            let mut statement=connection.prepare("SELECT a.id,a.sha256,a.relative_path,a.width,a.height FROM assets a WHERE (?1 IS NULL AND (EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=a.id) OR EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=a.id))) OR EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=a.id AND ca.case_id=?1)").map_err(sql)?;
            let rows = statement
            .query_map([case_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, u32>(3)?,
                    r.get::<_, u32>(4)?,
                ))
            })
            .map_err(sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql)?;
            drop(statement);
            rows
        } else {vec![]};
        let projects=if case_id.is_none(){view_catalog(&connection,"SELECT id,name,created_at,0,sort_order,sort_order FROM projects ORDER BY sort_order,created_at,id")?}else{vec![]};
        let tags=if case_id.is_none(){view_catalog(&connection,"SELECT id,name,created_at,is_quick,quick_order,sort_order FROM tags ORDER BY quick_order,created_at,id")?}else{vec![]};
        let trash_count=if case_id.is_none(){connection.query_row("SELECT count(*) FROM cases WHERE deleted_at IS NOT NULL",[],|r|r.get::<_,i64>(0)).map_err(sql)? as u64}else{0};
        let ui_preferences=crate::catalog::preferences_from(&connection)?;
        let dictionary=crate::catalog::dictionary_from(&connection)?;
        drop(connection);
        let output=self.asset_projection(rows,thumbnails)?;
        Ok(data::Library {
            cases,
            assets: output,
            projects,tags,trash_count,ui_preferences,dictionary,
        })
    }
    pub fn assets_for_cases(&self,ids:&[String])->Result<Vec<data::Asset>,String>{
        if ids.len()>120{return Err("case_asset_batch_too_large".into())}if ids.is_empty(){return Ok(vec![])}
        let connection=self.connection.lock().map_err(|e|e.to_string())?;
        let placeholders=(1..=ids.len()).map(|n|format!("?{n}")).collect::<Vec<_>>().join(",");
        let query=format!("SELECT DISTINCT a.id,a.sha256,a.relative_path,a.width,a.height FROM assets a JOIN case_assets ca ON ca.asset_id=a.id JOIN cases c ON c.id=ca.case_id WHERE c.deleted_at IS NULL AND c.id IN ({placeholders}) ORDER BY a.id");
        let mut statement=connection.prepare(&query).map_err(sql)?;
        let rows=statement.query_map(rusqlite::params_from_iter(ids),|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(sql)?.collect::<Result<Vec<AssetRow>,_>>().map_err(sql)?;
        drop(statement);drop(connection);
        self.asset_projection(rows,true)
    }
    pub(crate) fn asset_projection(&self,rows:Vec<AssetRow>,thumbnails:bool)->Result<Vec<data::Asset>,String>{
        // Decoding and filesystem work never hold the database connection mutex.
        let mut output = vec![];
        for (id, hash, relative, width, height) in rows {
            let original = self.paths.resolve(&relative, "assets")?;
            let thumbnail = if thumbnails {
                assets::thumbnail(&self.paths, &hash, &original)?
            } else {
                self.paths.destination(&DataPaths::thumb_relative(&hash)?)
            };
            output.push(data::Asset {
                id,
                sha256: hash,
                original: original.to_string_lossy().into_owned(),
                thumbnail: thumbnail.to_string_lossy().into_owned(),
                width,
                height,
            });
        }
        Ok(output)
    }
    pub fn check(&self) -> Result<serde_json::Value, String> {
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(sql)?;
        if integrity != "ok" {
            return Err(format!("integrity_check: {integrity}"));
        }
        let mut foreign = connection
            .prepare("PRAGMA foreign_key_check")
            .map_err(sql)?;
        if foreign
            .query([])
            .map_err(sql)?
            .next()
            .map_err(sql)?
            .is_some()
        {
            return Err("foreign_key_check_failed".into());
        }
        let references: i64 = connection
            .query_row("SELECT count(*) FROM case_assets", [], |r| r.get(0))
            .map_err(sql)?;
        let version: String = connection
            .query_row("SELECT sqlite_version()", [], |r| r.get(0))
            .map_err(sql)?;
        let case_count:i64=connection.query_row("SELECT count(*) FROM cases WHERE deleted_at IS NULL",[],|r|r.get(0)).map_err(sql)?;
        let rows={
            let mut statement=connection.prepare("SELECT a.id,a.sha256,a.extension,a.relative_path,a.gc_state,EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=a.id) OR EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=a.id) FROM assets a ORDER BY a.id").map_err(sql)?;
            let rows=statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,bool>(5)?))).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
            rows
        };
        drop(foreign);
        drop(connection);
        let asset_count=rows.len();
        let mut recoverable_missing=vec![];
        for (id,hash,extension,relative,gc_state,referenced) in rows {
            if relative!=DataPaths::asset_relative(&hash,&extension)? {return Err("invalid_stored_path".into())}
            let original=match self.paths.resolve(&relative,"assets") {
                Ok(path)=>path,
                Err(error)=>{
                    // A prior GC may have unlinked the file but failed to delete its pending row.
                    // Only true absence is recoverable: a dangling link or inaccessible path fails.
                    self.paths.validate_managed_area("assets")?;
                    if gc_state=="pending" && !referenced && matches!(std::fs::symlink_metadata(self.paths.destination(&relative)),Err(ref e) if e.kind()==std::io::ErrorKind::NotFound) {
                        recoverable_missing.push(id);continue;
                    }
                    return Err(error);
                }
            };
            if assets::hash_file(&original)?!=hash {return Err(format!("asset_hash_mismatch: {id}"))}
        }
        let hashes=if recoverable_missing.is_empty(){"ok"}else{"ok_except_recoverable_pending_missing"};
        Ok(
            serde_json::json!({"schemaVersion":SCHEMA_VERSION,"sqliteVersion":version,"cases":case_count,"assets":asset_count,"caseAssetReferences":references,"integrity":"ok","foreignKeys":"ok","originalHashes":hashes,"recoverablePendingMissing":recoverable_missing.len(),"recoverablePendingMissingIds":recoverable_missing}),
        )
    }
}
fn view_catalog(connection:&Connection,query:&str)->Result<Vec<data::CatalogItem>,String>{
    let mut statement=connection.prepare(query).map_err(sql)?;
    let rows=statement.query_map([],|r|Ok(data::CatalogItem{id:r.get(0)?,name:r.get(1)?,created_at:r.get(2)?,is_quick:r.get(3)?,quick_order:r.get(4)?,sort_order:r.get(5)?})).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
    Ok(rows)
}
fn catalog(
    tx: &Connection,
    table: &str,
    name: &str,
    known: &mut HashMap<String, String>,
    timestamp: i64,
) -> Result<String, String> {
    let key = name.trim().to_lowercase();
    if let Some(id) = known.get(&key) {
        return Ok(id.clone());
    }
    let catalog_id = id();
    // table comes only from the two hardcoded callers above.
    tx.execute(
        &format!("INSERT INTO {table}(id,name,name_key,created_at) VALUES(?1,?2,?3,?4)"),
        params![catalog_id, name.trim(), key, timestamp],
    )
    .map_err(sql)?;
    known.insert(key, catalog_id.clone());
    Ok(catalog_id)
}

#[cfg(test)]
mod tests;
