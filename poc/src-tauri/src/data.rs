use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Case {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub project: String,
    pub tags: Vec<String>,
    #[serde(rename = "favorite", alias = "fav")]
    pub favorite: bool,
    pub used: u64,
    pub result: Option<String>,
    pub refs: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Asset {
    pub id: String,
    pub original: String,
    #[serde(rename = "thumbnail", alias = "thumb")]
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub sha256: String,
}
#[derive(Clone, Serialize)]
pub struct Library {
    pub cases: Vec<Case>,
    pub assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    #[serde(default)]
    fixture_kind: String,
    cases: Vec<Case>,
    assets: Vec<Asset>,
}
impl Library {
    pub fn load(path: &Path) -> Result<Self, String> {
        let root = dunce::canonicalize(path.parent().ok_or("manifest_missing_parent")?)
            .map_err(|e| e.to_string())?;
        let raw = std::fs::read(path).map_err(|e| format!("manifest_read: {e}"))?;
        if raw.len() > 16 * 1024 * 1024 {
            return Err("manifest_too_large".into());
        }
        let manifest: Manifest =
            serde_json::from_slice(&raw).map_err(|e| format!("manifest_parse: {e}"))?;
        if manifest.schema_version != 1 {
            return Err("manifest_schema_unsupported".into());
        }
        let _kind = manifest.fixture_kind;
        let mut library = Self {
            cases: manifest.cases,
            assets: manifest.assets,
        };
        let mut ids = HashSet::new();
        for asset in &mut library.assets {
            if !ids.insert(asset.id.clone()) {
                return Err("duplicate_asset_id".into());
            }
            let original = contained(&root, &asset.original)?;
            let thumb = contained(&root, &asset.thumbnail)?;
            if original == thumb {
                return Err("thumbnail_must_be_independent".into());
            }
            if !asset.sha256.is_empty()
                && (asset.sha256.len() != 64
                    || !asset.sha256.chars().all(|c| c.is_ascii_hexdigit()))
            {
                return Err("invalid_sha256".into());
            }
            asset.original = original.to_string_lossy().into_owned();
            asset.thumbnail = thumb.to_string_lossy().into_owned();
        }
        let mut cases = HashSet::new();
        let originals: HashSet<_> = library.assets.iter().map(|a| &a.original).collect();
        if library
            .assets
            .iter()
            .any(|a| originals.contains(&a.thumbnail))
        {
            return Err("thumbnail_cannot_expose_any_original".into());
        }
        for case in &library.cases {
            if !cases.insert(&case.id) {
                return Err("duplicate_case_id".into());
            }
            for id in case.result.iter().chain(&case.refs) {
                if !ids.contains(id) {
                    return Err("unknown_asset_reference".into());
                }
            }
        }
        Ok(library)
    }
    pub fn asset_path(&self, id: &str) -> Result<PathBuf, String> {
        let asset = self
            .assets
            .iter()
            .find(|a| a.id == id)
            .ok_or("unknown_asset")?;
        Ok(PathBuf::from(&asset.original))
    }
    pub fn verify_hashes(&self) -> Result<(), String> {
        for asset in &self.assets {
            if !asset.sha256.is_empty() {
                let bytes = std::fs::read(&asset.original).map_err(|e| e.to_string())?;
                if format!("{:x}", Sha256::digest(&bytes)) != asset.sha256.to_lowercase() {
                    return Err(format!("asset_hash_mismatch: {}", asset.id));
                }
            }
        }
        Ok(())
    }
    pub fn demo() -> Self {
        Self{assets:vec![],cases:vec![Case{id:"demo-text".into(),title:"通用产品高清精修 · PoC".into(),prompt:"【非敏感 PoC 测试】严格保留产品构图、比例与标签，优化真实阴影及材质细节。\n这一行用于验证完整多行文字粘贴。".into(),project:"技术验证".into(),tags:vec!["产品精修".into()],favorite:true,used:1,result:None,refs:vec![]}]}
    }
}
fn contained(root: &Path, value: &str) -> Result<PathBuf, String> {
    let relative = Path::new(value);
    if relative.is_absolute()
        || relative.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err("asset_path_must_be_relative".into());
    }
    let path =
        dunce::canonicalize(root.join(relative)).map_err(|e| format!("asset_not_found: {e}"))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err("asset_path_escapes_root".into());
    }
    Ok(path)
}
pub fn locate() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PROMPT_SHELF_MANIFEST") {
        return Some(PathBuf::from(path));
    }
    [
        PathBuf::from("fixtures/100x500/manifest.json"),
        PathBuf::from("../fixtures/100x500/manifest.json"),
        PathBuf::from("../../fixtures/100x500/manifest.json"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_absolute_or_traversal() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        assert!(contained(&root, "../escape").is_err());
        assert!(contained(&root, &root.to_string_lossy()).is_err());
    }
    #[test]
    fn aliases_match_controller_schema() {
        let c:Case=serde_json::from_str(r#"{"id":"c","title":"t","prompt":"p","project":"x","tags":[],"fav":true,"used":0,"result":null,"refs":[]}"#).unwrap();
        assert!(c.favorite);
    }
    #[test]
    fn controller_fixtures_load_100_cases_500_assets_and_hashes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["100x500", "100x500-mixed"] {
            let manifest = root.join("fixtures").join(name).join("manifest.json");
            if !manifest.is_file() {
                eprintln!("fixture unavailable; this test requires controller fixture: {name}");
                continue;
            }
            let library = Library::load(&manifest).unwrap();
            assert_eq!(library.cases.len(), 100);
            assert_eq!(library.assets.len(), 500);
            library.verify_hashes().unwrap();
            assert!(library
                .cases
                .iter()
                .any(|c| c.result.is_none() && c.refs.is_empty()));
            assert!(library.assets.iter().all(|a| a.original != a.thumbnail));
        }
    }
}
