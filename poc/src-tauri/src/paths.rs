use std::path::{Component, Path, PathBuf};

/// The only owner of on-disk layout. Stored paths are relative to this root.
#[derive(Clone, Debug)]
pub struct DataPaths {
    root: PathBuf,
}
impl DataPaths {
    pub fn new(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let paths = Self {
            root: dunce::canonicalize(root).map_err(|e| e.to_string())?,
        };
        for relative in ["assets", "cache", "cache/thumbs", "staging", "logs"] {
            let path = paths.root.join(relative);
            if !path.exists() {
                std::fs::create_dir(&path).map_err(|e| e.to_string())?;
            }
            let real = dunce::canonicalize(&path).map_err(|e| e.to_string())?;
            if real != path || !real.is_dir() {
                return Err("data_directory_link_rejected".into());
            }
        }
        Ok(paths)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn database(&self) -> Result<PathBuf, String> {
        self.local_file("library.db")
    }
    pub fn lock(&self) -> Result<PathBuf, String> {
        self.local_file(".library.lock")
    }
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    fn local_file(&self, relative: &str) -> Result<PathBuf, String> {
        let path = self.root.join(relative);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !dunce::canonicalize(&path)
                        .map_err(|e| e.to_string())?
                        .starts_with(&self.root)
                {
                    return Err("data_file_link_rejected".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(path)
    }
    pub fn resolve(&self, relative: &str, area: &str) -> Result<PathBuf, String> {
        let path = Path::new(relative);
        if path.is_absolute()
            || !path.components().all(|c| matches!(c, Component::Normal(_)))
            || !path.starts_with(area)
        {
            return Err("invalid_stored_path".into());
        }
        let real = dunce::canonicalize(self.root.join(path))
            .map_err(|e| format!("stored_file_missing: {e}"))?;
        let allowed = dunce::canonicalize(self.root.join(area)).map_err(|e| e.to_string())?;
        if !real.starts_with(&self.root) || !real.starts_with(allowed) || !real.is_file() {
            return Err("stored_path_escapes_root".into());
        }
        Ok(real)
    }
    pub fn asset_relative(hash: &str, ext: &str) -> Result<String, String> {
        if hash.len() != 64
            || !hash.bytes().all(|c| c.is_ascii_hexdigit())
            || !["png", "jpg", "webp"].contains(&ext)
        {
            return Err("invalid_asset_name".into());
        }
        Ok(format!("assets/{hash}.{ext}"))
    }
    pub fn thumb_relative(hash: &str) -> Result<String, String> {
        Self::asset_relative(hash, "png")?;
        Ok(format!("cache/thumbs/{hash}-180-v1.png"))
    }
    pub(crate) fn destination(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
    /// Revalidate managed directories at the point of filesystem mutation. In particular,
    /// do not follow a junction inserted after DataPaths::new, including an internal link.
    pub(crate) fn validate_managed_area(&self, area: &str) -> Result<PathBuf, String> {
        let relative = Path::new(area);
        if relative.is_absolute() || !relative.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err("invalid_managed_area".into());
        }
        let mut current = self.root.clone();
        for part in relative.components() {
            current.push(part.as_os_str());
            let metadata = std::fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
            if linked(&metadata) || !metadata.is_dir() || dunce::canonicalize(&current).map_err(|e| e.to_string())? != current {
                return Err("data_directory_link_rejected".into());
            }
        }
        Ok(current)
    }
    /// Only a known DB asset/cache file may be removed. Missing files are an idempotent success.
    pub(crate) fn remove_managed_file(&self, relative: &str, area: &str) -> Result<(), String> {
        let area_path = self.validate_managed_area(area)?;
        let relative = Path::new(relative);
        if relative.is_absolute() || !relative.components().all(|c| matches!(c, Component::Normal(_)))
            || relative.parent() != Some(Path::new(area)) {
            return Err("invalid_gc_path".into());
        }
        let path = area_path.join(relative.file_name().ok_or("invalid_gc_path")?);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if linked(&metadata) || !metadata.is_file() || dunce::canonicalize(&path).map_err(|e| e.to_string())? != path {
                    return Err("data_file_link_rejected".into());
                }
                std::fs::remove_file(&path).map_err(|e| format!("gc_remove_failed: {e}"))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}
fn linked(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))] { metadata.file_type().is_symlink() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_files_allow_new_files_and_reject_dangling_links_when_supported() {
        let directory = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(directory.path()).unwrap();
        assert!(paths.database().is_ok());
        assert!(paths.lock().is_ok());
        let missing = directory.path().join("absent-target");
        for relative in ["library.db", ".library.lock"] {
            let link = directory.path().join(relative);
            #[cfg(windows)]
            let creation = std::os::windows::fs::symlink_file(&missing, &link);
            #[cfg(unix)]
            let creation = std::os::unix::fs::symlink(&missing, &link);
            if let Err(error) = creation {
                #[cfg(windows)]
                if error.raw_os_error() == Some(1314) {
                    eprintln!("SYMLINK_UNAVAILABLE: Windows privilege not available; dangling-link branch not exercised");
                    return;
                }
                panic!("symlink setup failed: {error}");
            }
            assert!(!link.exists());
            assert!(paths
                .local_file(relative)
                .unwrap_err()
                .contains("data_file_link_rejected"));
            assert!(!missing.exists());
            std::fs::remove_file(link).unwrap();
        }
        eprintln!(
            "SYMLINK_VERIFIED: dangling database and lock links rejected without creating targets"
        );
    }
}
