//! One data-root lease and one active library. Lock order: host -> edit session -> resources -> DB.
//! Restores switch a durable pointer, never move the legacy flat library or delete an old storage.
use crate::{paths::DataPaths, store::Store};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        RwLock,
    },
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Location {
    Flat,
    Storage { id: String },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pointer {
    version: u32,
    generation: u64,
    location: Location,
}
struct Active {
    pointer: Pointer,
    store: Store,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostContext {
    pub generation: u64,
    pub revision: u64,
    pub maintenance: bool,
}

pub struct LibraryHost {
    root: PathBuf,
    active: RwLock<Active>,
    maintenance: AtomicBool,
    revision: AtomicU64,
    // The Store still owns its storage-local lease. This root lease also excludes another
    // E GUI/CLI which would otherwise open a different active storage in the same root.
    _lease: File,
}
pub struct MaintenanceGuard<'a> {
    host: &'a LibraryHost,
    previous: Option<Pointer>,
}

impl LibraryHost {
    pub fn open(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let root = dunce::canonicalize(root).map_err(|e| e.to_string())?;
        let lease_path = root.join(".library-host.lock");
        reject_link(&lease_path)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lease_path)
            .map_err(|e| e.to_string())?;
        lease
            .try_lock()
            .map_err(|e| format!("library_in_use: {e}"))?;

        let current_path = root.join("active.json");
        let previous_path = root.join("active.previous.json");
        let pointer_exists = std::fs::symlink_metadata(&current_path).is_ok()
            || std::fs::symlink_metadata(&previous_path).is_ok();
        let active = if pointer_exists {
            let current_pointer = read_pointer(&root, &current_path);
            let current_generation = current_pointer.as_ref().ok().map(|p| p.generation);
            match current_pointer.and_then(|p| open_pointer(&root, p)) {
                Ok(active) => active,
                Err(current_error) => {
                    let mut active = read_pointer(&root, &previous_path).and_then(|p| open_pointer(&root, p))
                        .map_err(|previous_error| format!("library_recovery_required: current={current_error}; previous={previous_error}"))?;
                    active.pointer.generation = active
                        .pointer
                        .generation
                        .max(current_generation.unwrap_or(0))
                        .checked_add(1)
                        .ok_or("generation_overflow")?;
                    write_pointer(&root, "active.json", &active.pointer)?;
                    active
                }
            }
        } else {
            // With no pointer, retain the A-D layout exactly where it is. An existing data
            // directory with assets but a missing DB is a damaged library, not a first launch.
            if std::fs::symlink_metadata(root.join("library.db")).is_ok() {
                check_existing_database(&root.join("library.db"))?;
            }
            if !root.join("library.db").exists()
                && root.join("assets").is_dir()
                && std::fs::read_dir(root.join("assets"))
                    .map_err(|e| e.to_string())?
                    .next()
                    .is_some()
            {
                return Err("library_recovery_required: flat_database_missing".into());
            }
            let pointer = Pointer {
                version: 1,
                generation: 1,
                location: Location::Flat,
            };
            let store = Store::open(DataPaths::new(&root)?)?;
            store.check()?;
            Active { pointer, store }
        };
        Ok(Self {
            root,
            active: RwLock::new(active),
            maintenance: AtomicBool::new(false),
            revision: AtomicU64::new(0),
            _lease: lease,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }
    pub fn storage_parent(&self) -> Result<PathBuf, String> {
        let path = self.root.join("storage");
        if !path.exists() {
            std::fs::create_dir(&path).map_err(|e| e.to_string())?;
        }
        reject_link(&path)?;
        if !path.is_dir() || dunce::canonicalize(&path).map_err(|e| e.to_string())? != path {
            return Err("invalid_storage_directory".into());
        }
        Ok(path)
    }
    pub fn context(&self) -> Result<HostContext, String> {
        let active = self.active.read().map_err(|e| e.to_string())?;
        Ok(self.context_for(active.pointer.generation))
    }
    fn context_for(&self, generation: u64) -> HostContext {
        HostContext {
            generation,
            revision: self.revision.load(Ordering::Acquire),
            maintenance: self.maintenance.load(Ordering::Acquire),
        }
    }

    pub fn read<T>(
        &self,
        generation: u64,
        action: impl FnOnce(&Store) -> Result<T, String>,
    ) -> Result<T, String> {
        let active = self.active.read().map_err(|e| e.to_string())?;
        require_generation(active.pointer.generation, generation)?;
        action(&active.store)
    }
    /// A short coherent preview: exclude business mutations until the summary and revision
    /// have both been observed. Long ZIP preflight work happens after this guard is released.
    pub fn observe<T>(
        &self,
        generation: u64,
        action: impl FnOnce(&Store) -> Result<T, String>,
    ) -> Result<(T, HostContext), String> {
        let active = self.active.write().map_err(|e| e.to_string())?;
        require_generation(active.pointer.generation, generation)?;
        let result = action(&active.store)?;
        Ok((result, self.context_for(active.pointer.generation)))
    }
    pub fn write<T>(
        &self,
        generation: u64,
        action: impl FnOnce(&Store) -> Result<T, String>,
    ) -> Result<T, String> {
        self.mutate(generation, false, action)
    }
    /// Only an already-running queue's first-dispatch accounting may finish during maintenance.
    pub fn ongoing_write<T>(
        &self,
        generation: u64,
        action: impl FnOnce(&Store) -> Result<T, String>,
    ) -> Result<T, String> {
        self.mutate(generation, true, action)
    }
    fn mutate<T>(
        &self,
        generation: u64,
        ongoing: bool,
        action: impl FnOnce(&Store) -> Result<T, String>,
    ) -> Result<T, String> {
        let active = self.active.read().map_err(|e| e.to_string())?;
        require_generation(active.pointer.generation, generation)?;
        if !ongoing && self.maintenance.load(Ordering::Acquire) {
            return Err("library_maintenance_in_progress".into());
        }
        let output = action(&active.store)?;
        self.revision.fetch_add(1, Ordering::AcqRel);
        Ok(output)
    }
    pub fn begin_maintenance(&self, generation: u64) -> Result<MaintenanceGuard<'_>, String> {
        self.maintenance
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "library_maintenance_in_progress".to_string())?;
        let result = (|| {
            // Drain operations which passed the gate before maintenance was set. Ordinary
            // reads may resume after this barrier; new writes/output starts stay blocked.
            let active = self.active.write().map_err(|e| e.to_string())?;
            require_generation(active.pointer.generation, generation)
        })();
        if let Err(error) = result {
            self.maintenance.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(MaintenanceGuard {
            host: self,
            previous: None,
        })
    }
}

impl MaintenanceGuard<'_> {
    /// `storage` comes from the completed backup preflight's into_storage(), not a WebView path.
    /// External ZIP/schema/image checks have already run once at that boundary.
    pub fn install(
        &mut self,
        storage: &Path,
        expected_revision: u64,
    ) -> Result<HostContext, String> {
        self.install_inner(storage, expected_revision, |_| Ok(()))
    }
    fn install_inner(
        &mut self,
        storage: &Path,
        expected_revision: u64,
        checkpoint: impl Fn(SwitchPoint) -> Result<(), String>,
    ) -> Result<HostContext, String> {
        let mut active = self.host.active.write().map_err(|e| e.to_string())?;
        if self.host.revision.load(Ordering::Acquire) != expected_revision {
            return Err("restore_preview_changed_reconfirm".into());
        }
        let id = storage
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("invalid_prepared_storage")?
            .to_owned();
        // This checks the typed preflight's placement contract, not the package again.
        if storage.parent() != Some(self.host.storage_parent()?.as_path()) {
            return Err("invalid_prepared_storage".into());
        }
        let store = Store::open(DataPaths::new(storage)?)?;
        let previous = active.pointer.clone();
        let next = Pointer {
            version: 1,
            generation: previous
                .generation
                .checked_add(1)
                .ok_or("generation_overflow")?,
            location: Location::Storage { id },
        };
        write_pointer(&self.host.root, "active.previous.json", &previous)?;
        checkpoint(SwitchPoint::PreviousDurable)?;
        if let Err(error) = write_pointer(&self.host.root, "active.json", &next)
            .and_then(|_| checkpoint(SwitchPoint::ActiveDurable))
        {
            write_pointer(&self.host.root, "active.json", &previous).map_err(|rollback| {
                format!("restore_pointer_recovery_required: switch={error}; rollback={rollback}")
            })?;
            return Err(error);
        }
        let old = std::mem::replace(
            &mut *active,
            Active {
                pointer: next,
                store,
            },
        );
        self.previous = Some(previous);
        drop(old); // Close the old DB/lease; its directory and output originals remain intact.
        self.host.revision.fetch_add(1, Ordering::AcqRel);
        Ok(self.host.context_for(active.pointer.generation))
    }

    /// The caller keeps the maintenance guard through thumbnail-scope/window initialization.
    /// If that initialization fails, reopen the previous complete library and switch back.
    pub fn rollback(&mut self) -> Result<HostContext, String> {
        let previous = self
            .previous
            .clone()
            .ok_or("restore_has_not_switched".to_string())?;
        let mut active = self.host.active.write().map_err(|e| e.to_string())?;
        let reopened = open_pointer(&self.host.root, previous)?;
        let pointer = Pointer {
            generation: active
                .pointer
                .generation
                .checked_add(1)
                .ok_or("generation_overflow")?,
            ..reopened.pointer
        };
        write_pointer(&self.host.root, "active.json", &pointer)?;
        let old = std::mem::replace(
            &mut *active,
            Active {
                pointer,
                store: reopened.store,
            },
        );
        drop(old);
        self.previous = None;
        self.host.revision.fetch_add(1, Ordering::AcqRel);
        Ok(self.host.context_for(active.pointer.generation))
    }
}
impl Drop for MaintenanceGuard<'_> {
    fn drop(&mut self) {
        self.host.maintenance.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
enum SwitchPoint {
    PreviousDurable,
    ActiveDurable,
}
fn require_generation(actual: u64, expected: u64) -> Result<(), String> {
    if actual != expected {
        Err("stale_library_generation".into())
    } else {
        Ok(())
    }
}
fn read_pointer(root: &Path, path: &Path) -> Result<Pointer, String> {
    reject_link(path)?;
    let mut file = File::open(path).map_err(|e| format!("pointer_read: {e}"))?;
    let mut bytes = vec![];
    Read::by_ref(&mut file)
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("invalid_active_pointer".into());
    }
    let pointer: Pointer =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid_active_pointer: {e}"))?;
    if pointer.version != 1 || pointer.generation == 0 {
        return Err("unsupported_active_pointer".into());
    }
    if let Location::Storage { id } = &pointer.location {
        let parsed = uuid::Uuid::parse_str(id).map_err(|_| "invalid_storage_id")?;
        if parsed.hyphenated().to_string() != *id {
            return Err("invalid_storage_id".into());
        }
        reject_link(&root.join("storage"))?;
    }
    Ok(pointer)
}
fn location_path(root: &Path, location: &Location) -> PathBuf {
    match location {
        Location::Flat => root.to_owned(),
        Location::Storage { id } => root.join("storage").join(id),
    }
}
fn open_pointer(root: &Path, pointer: Pointer) -> Result<Active, String> {
    let path = location_path(root, &pointer.location);
    reject_link(&path)?;
    if dunce::canonicalize(&path).map_err(|e| e.to_string())? != path {
        return Err("invalid_storage_directory".into());
    }
    // Never call Store::open for a missing pointed-to database: it would create an empty one.
    let database = path.join("library.db");
    check_existing_database(&database)?;
    let store = Store::open(DataPaths::new(&path)?)?;
    store.check()?;
    Ok(Active { pointer, store })
}
/// Existing roots are data, never a new-library initialization request. Shared by the
/// flat compatibility path and active/previous pointer resolution at this entry boundary.
fn check_existing_database(database: &Path) -> Result<(), String> {
    reject_link(database)?;
    if !database.is_file() {
        return Err("library_database_missing".into());
    }
    let connection =
        rusqlite::Connection::open_with_flags(database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("existing_library_unreadable: {e}"))?;
    let schema: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("existing_library_unreadable: {e}"))?;
    if schema == 0 {
        return Err("library_recovery_required: existing_library_unversioned".into());
    }
    Ok(())
}
fn write_pointer(root: &Path, name: &str, pointer: &Pointer) -> Result<(), String> {
    let target = root.join(name);
    reject_link(&target)?;
    let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
    serde_json::to_writer(file.as_file_mut(), pointer).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(&target)
        .map_err(|e| format!("pointer_replace: {e}"))?
        .sync_all()
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn reject_link(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                metadata.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let linked = metadata.file_type().is_symlink();
            if linked {
                return Err("data_path_link_rejected".into());
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
