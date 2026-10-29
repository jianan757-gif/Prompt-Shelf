//! ZIP backup boundary: validate external packages once, then pass a prepared storage object.
use crate::store::{Store, SCHEMA_VERSION};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::{NamedTempFile, TempDir};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

mod validate;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Summary {
    pub cases: u64,
    pub trash: u64,
    pub tags: u64,
    pub projects: u64,
    pub drafts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileRecord {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub schema_version: i64,
    pub app_version: String,
    pub app_revision: String,
    pub created_at: i64,
    pub summary: Summary,
    pub files: Vec<FileRecord>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub operation_id: String,
    pub phase: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: u64,
    pub files_total: u64,
}
pub struct JobControl {
    operation_id: String,
    // 0 = running, 1 = cancelled, 2 = commit has started.
    phase: AtomicU8,
}
impl JobControl {
    pub fn new(operation_id: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            phase: AtomicU8::new(0),
        }
    }
    pub fn cancel(&self) -> bool {
        match self
            .phase
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(phase) => phase == 1,
        }
    }
    pub fn begin_commit(&self) -> Result<(), String> {
        match self
            .phase
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            Err(1) => Err("backup_cancelled".into()),
            Err(_) => Err("backup_commit_already_started".into()),
        }
    }
    pub fn check(&self) -> Result<(), String> {
        if self.phase.load(Ordering::Acquire) == 1 {
            Err("backup_cancelled".into())
        } else {
            Ok(())
        }
    }
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
}

/// Owns only a directory created by this preflight. Dropping a cancelled preview removes it.
/// The manifest and original files have passed all package boundary validation.
pub struct PreparedRestore {
    directory: TempDir,
    manifest: Manifest,
}
impl PreparedRestore {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn summary(&self) -> &Summary {
        &self.manifest.summary
    }
    pub fn into_storage(self) -> Result<PathBuf, String> {
        let path = self
            .directory
            .path()
            .parent()
            .ok_or("restore_parent_missing")?
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::rename(self.directory.path(), &path).map_err(|e| e.to_string())?;
        // The path is now owned by the host, even if a later pointer install fails.
        let _old = self.directory.keep();
        Ok(path)
    }
}

fn sql(error: rusqlite::Error) -> String {
    format!("sqlite: {error}")
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub(crate) fn summary_from(c: &Connection) -> Result<Summary, String> {
    c.query_row("SELECT (SELECT count(*) FROM cases WHERE deleted_at IS NULL),(SELECT count(*) FROM cases WHERE deleted_at IS NOT NULL),(SELECT count(*) FROM tags),(SELECT count(*) FROM projects),(SELECT count(*) FROM drafts)",[],|r|Ok(Summary{cases:r.get::<_,i64>(0)? as u64,trash:r.get::<_,i64>(1)? as u64,tags:r.get::<_,i64>(2)? as u64,projects:r.get::<_,i64>(3)? as u64,drafts:r.get::<_,i64>(4)? as u64})).map_err(sql)
}
pub fn summary(store: &Store) -> Result<Summary, String> {
    summary_from(&*store.connection.lock().map_err(|e| e.to_string())?)
}

struct Tracker<'a, F: FnMut(Progress)> {
    control: &'a JobControl,
    emit: F,
    progress: Progress,
    last_bytes: u64,
}
impl<'a, F: FnMut(Progress)> Tracker<'a, F> {
    fn new(control: &'a JobControl, emit: F, phase: &str, total: u64, files: u64) -> Self {
        Self {
            control,
            emit,
            progress: Progress {
                operation_id: control.operation_id.clone(),
                phase: phase.into(),
                bytes_done: 0,
                bytes_total: total,
                files_done: 0,
                files_total: files,
            },
            last_bytes: 0,
        }
    }
    fn emit(&mut self) {
        (self.emit)(self.progress.clone());
        self.last_bytes = self.progress.bytes_done;
    }
    fn add(&mut self, bytes: u64) -> Result<(), String> {
        self.control.check()?;
        self.progress.bytes_done = self
            .progress
            .bytes_done
            .checked_add(bytes)
            .ok_or("backup_size_overflow")?;
        if self.progress.bytes_done.saturating_sub(self.last_bytes) >= 1024 * 1024 {
            self.emit()
        }
        Ok(())
    }
    fn file(&mut self) {
        self.progress.files_done += 1;
        self.emit()
    }
}

fn copy_hash<R: Read, W: Write, F: FnMut(Progress)>(
    reader: &mut R,
    writer: &mut W,
    expected: u64,
    tracker: &mut Tracker<'_, F>,
) -> Result<String, String> {
    let mut total = 0u64;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        tracker.control.check()?;
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or("backup_size_overflow")?;
        if total > expected {
            return Err("backup_actual_bytes_exceed_manifest".into());
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        hash.update(&buffer[..read]);
        tracker.add(read as u64)?;
    }
    if total != expected {
        return Err("backup_file_size_mismatch".into());
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl Store {
    /// The host holds maintenance rights. Snapshot setup coordinates with imports and GC;
    /// the host then keeps immutable originals stable while streaming without Store locks.
    pub fn export_backup(
        &self,
        destination: &Path,
        control: &JobControl,
        mut emit: impl FnMut(Progress),
    ) -> Result<Manifest, String> {
        control.check()?;
        let _resources = self.resources.lock().map_err(|e| e.to_string())?;
        let scratch = tempfile::Builder::new()
            .prefix("backup-")
            .tempdir_in(self.paths().staging())
            .map_err(|e| e.to_string())?;
        let db_path = scratch.path().join("library.db");
        {
            let source = self.connection.lock().map_err(|e| e.to_string())?;
            let pages: u64 = source
                .pragma_query_value(None, "page_count", |r| Ok(r.get::<_, i64>(0)? as u64))
                .map_err(sql)?;
            let page_size: u64 = source
                .pragma_query_value(None, "page_size", |r| Ok(r.get::<_, i64>(0)? as u64))
                .map_err(sql)?;
            let required = pages
                .checked_mul(page_size)
                .and_then(|n| n.checked_mul(2))
                .ok_or("backup_size_overflow")?;
            if fs2::available_space(scratch.path()).map_err(|e| e.to_string())? < required {
                return Err("backup_insufficient_space".into());
            }
            let mut snapshot = Connection::open(&db_path).map_err(sql)?;
            {
                let backup = Backup::new(&source, &mut snapshot).map_err(sql)?;
                loop {
                    control.check()?;
                    let step = backup.step(128).map_err(sql)?;
                    let progress = backup.progress();
                    emit(Progress {
                        operation_id: control.operation_id.clone(),
                        phase: "snapshot".into(),
                        bytes_done: (progress.pagecount - progress.remaining).max(0) as u64
                            * page_size,
                        bytes_total: progress.pagecount.max(0) as u64 * page_size,
                        files_done: 0,
                        files_total: 1,
                    });
                    match step {
                        StepResult::Done => break,
                        StepResult::More => {}
                        StepResult::Busy | StepResult::Locked => {
                            return Err("backup_snapshot_busy_retry".into())
                        }
                        _ => return Err("backup_snapshot_incomplete".into()),
                    }
                }
            }
            snapshot
                .pragma_update(None, "journal_mode", "DELETE")
                .map_err(sql)?;
            snapshot
                .pragma_update(None, "foreign_keys", true)
                .map_err(sql)?;
            let tx = snapshot.transaction().map_err(sql)?;
            tx.execute("DELETE FROM assets WHERE NOT EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=assets.id) AND NOT EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=assets.id)",[]).map_err(sql)?;
            tx.execute(
                "UPDATE assets SET gc_state='live' WHERE gc_state!='live'",
                [],
            )
            .map_err(sql)?;
            tx.commit().map_err(sql)?;
        }
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&db_path)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        let snapshot =
            Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(sql)?;
        let summary = summary_from(&snapshot)?;
        let records = {
            let mut q = snapshot
                .prepare("SELECT relative_path,byte_size,sha256 FROM assets ORDER BY relative_path")
                .map_err(sql)?;
            let rows = q
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)? as u64,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(sql)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql)?;
            rows
        };
        drop(snapshot);
        let db_bytes = std::fs::metadata(&db_path)
            .map_err(|e| e.to_string())?
            .len();
        let mut sources = vec![("library.db".to_owned(), db_path, db_bytes, None)];
        for (relative, bytes, hash) in records {
            sources.push((
                relative.clone(),
                self.paths().resolve(&relative, "assets")?,
                bytes,
                Some(hash),
            ));
        }
        drop(_resources);
        let total = sources.iter().try_fold(0u64, |sum, (_, _, n, _)| {
            sum.checked_add(*n).ok_or("backup_size_overflow")
        })?;
        let mut manifest = Manifest {
            format: "prompt-shelf-backup".into(),
            version: 1,
            schema_version: SCHEMA_VERSION,
            app_version: env!("CARGO_PKG_VERSION").into(),
            app_revision: "4.4-e".into(),
            created_at: now(),
            summary,
            files: sources
                .iter()
                .map(|(name, _, bytes, _)| FileRecord {
                    path: name.clone(),
                    bytes: *bytes,
                    sha256: "0".repeat(64),
                })
                .collect(),
        };
        let manifest_estimate = serde_json::to_vec(&manifest)
            .map_err(|e| e.to_string())?
            .len() as u64;
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or("backup_destination_parent_required")?;
        let required = total
            .checked_add(manifest_estimate)
            .and_then(|n| n.checked_add(sources.len() as u64 * 256 + 4096))
            .ok_or("backup_size_overflow")?;
        if fs2::available_space(parent).map_err(|e| e.to_string())? < required {
            return Err("backup_insufficient_space".into());
        }
        let mut temporary = NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let mut tracker = Tracker::new(
            control,
            emit,
            "archive",
            total + manifest_estimate,
            sources.len() as u64 + 1,
        );
        tracker.emit();
        {
            let mut writer = ZipWriter::new(temporary.as_file_mut());
            for ((relative, path, bytes, expected), record) in
                sources.iter().zip(&mut manifest.files)
            {
                control.check()?;
                writer
                    .start_file(relative, zip_options(*bytes))
                    .map_err(|e| e.to_string())?;
                let mut source = File::open(path).map_err(|e| e.to_string())?;
                let hash = copy_hash(&mut source, &mut writer, *bytes, &mut tracker)?;
                if expected.as_ref().is_some_and(|expected| expected != &hash) {
                    return Err("backup_original_hash_mismatch".into());
                }
                record.sha256 = hash;
                tracker.file();
            }
            let raw = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
            writer
                .start_file("manifest.json", zip_options(raw.len() as u64))
                .map_err(|e| e.to_string())?;
            let _hash = copy_hash(
                &mut raw.as_slice(),
                &mut writer,
                raw.len() as u64,
                &mut tracker,
            )?;
            tracker.file();
            writer.finish().map_err(|e| e.to_string())?;
        }
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        control.begin_commit()?;
        tracker.progress.phase = "publishing".into();
        tracker.emit();
        temporary
            .persist_noclobber(destination)
            .map_err(|e| format!("backup_publish_failed: {e}"))?;
        tracker.progress.phase = "completed".into();
        tracker.emit();
        Ok(manifest)
    }
}

fn zip_options(bytes: u64) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(bytes >= zip::ZIP64_BYTES_THR)
        .unix_permissions(0o600)
}

pub fn prepare_restore(
    zip_path: &Path,
    storage_parent: &Path,
    control: &JobControl,
    emit: impl FnMut(Progress),
) -> Result<PreparedRestore, String> {
    prepare_with_space(zip_path, storage_parent, control, emit, None)
}

fn prepare_with_space(
    zip_path: &Path,
    storage_parent: &Path,
    control: &JobControl,
    emit: impl FnMut(Progress),
    space_override: Option<u64>,
) -> Result<PreparedRestore, String> {
    control.check()?;
    let source = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(source.try_clone().map_err(|e| e.to_string())?)
        .map_err(|e| format!("backup_zip_invalid: {e}"))?;
    let names = validate::archive_entries(&mut archive, source)?;
    let manifest_index = *names
        .get("manifest.json")
        .ok_or("backup_manifest_missing")?;
    let raw = {
        let mut entry = archive
            .by_index(manifest_index)
            .map_err(|e| e.to_string())?;
        let mut raw = vec![];
        entry
            .by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        if raw.len() > 16 * 1024 * 1024 {
            return Err("backup_manifest_too_large".into());
        }
        raw
    };
    let manifest: Manifest =
        serde_json::from_slice(&raw).map_err(|e| format!("backup_manifest_invalid: {e}"))?;
    let total = validate::manifest(&manifest, &names, &mut archive)?;
    let available = space_override
        .map(Ok)
        .unwrap_or_else(|| fs2::available_space(storage_parent))
        .map_err(|e| e.to_string())?;
    // A second DB-sized allowance covers known migration journals and opening the new storage.
    let db_bytes = manifest
        .files
        .iter()
        .find(|r| r.path == "library.db")
        .ok_or("backup_database_missing")?
        .bytes;
    if available < total.checked_add(db_bytes).ok_or("backup_size_overflow")? {
        return Err("backup_insufficient_space".into());
    }
    let directory = tempfile::Builder::new()
        .prefix("restore-")
        .tempdir_in(storage_parent)
        .map_err(|e| e.to_string())?;
    std::fs::create_dir(directory.path().join("assets")).map_err(|e| e.to_string())?;
    let mut tracker = Tracker::new(control, emit, "extract", total, manifest.files.len() as u64);
    tracker.emit();
    for record in &manifest.files {
        control.check()?;
        let index = names[&record.path];
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let mut file =
            File::create(directory.path().join(&record.path)).map_err(|e| e.to_string())?;
        let hash = copy_hash(&mut entry, &mut file, record.bytes, &mut tracker)?;
        if hash != record.sha256 {
            return Err(format!("backup_hash_mismatch: {}", record.path));
        }
        file.sync_all().map_err(|e| e.to_string())?;
        tracker.file();
    }
    control.check()?;
    tracker.progress.phase = "validate".into();
    tracker.emit();
    validate::database(directory.path(), &manifest, control)?;
    tracker.progress.phase = "ready".into();
    tracker.emit();
    Ok(PreparedRestore {
        directory,
        manifest,
    })
}

#[cfg(test)]
mod tests;
