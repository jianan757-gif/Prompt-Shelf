//! All external-package checks live here and run before PreparedRestore is constructed.
use super::*;
use crate::{catalog, paths::DataPaths};
use rusqlite::OpenFlags;
use std::{
    collections::{BTreeMap, HashSet},
    io::{Seek, SeekFrom},
};

fn entry_name(name: &str) -> Result<(), String> {
    if name == "manifest.json" || name == "library.db" {
        return Ok(());
    }
    let tail = name
        .strip_prefix("assets/")
        .ok_or("backup_entry_path_invalid")?;
    let (hash, extension) = tail.rsplit_once('.').ok_or("backup_entry_path_invalid")?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !["png", "jpg", "webp"].contains(&extension)
    {
        return Err("backup_entry_path_invalid".into());
    }
    Ok(())
}

pub(super) fn archive_entries(
    archive: &mut ZipArchive<File>,
    mut source: File,
) -> Result<BTreeMap<String, usize>, String> {
    if archive.offset() != 0 {
        return Err("backup_zip_prefix_not_supported".into());
    }
    // zip's indexed view coalesces equal raw names. Inspect central headers once so an
    // exact duplicate cannot disappear before the format's duplicate-path check.
    source
        .seek(SeekFrom::Start(archive.central_directory_start()))
        .map_err(|e| e.to_string())?;
    let mut central_names = HashSet::new();
    let mut count = 0usize;
    loop {
        let mut signature = [0u8; 4];
        source
            .read_exact(&mut signature)
            .map_err(|e| e.to_string())?;
        if signature != *b"PK\x01\x02" {
            if [*b"PK\x05\x06", *b"PK\x06\x06"].contains(&signature) {
                break;
            }
            return Err("backup_central_directory_invalid".into());
        }
        let mut header = [0u8; 42];
        source.read_exact(&mut header).map_err(|e| e.to_string())?;
        let name_len = u16::from_le_bytes(header[24..26].try_into().unwrap()) as usize;
        let extra = u16::from_le_bytes(header[26..28].try_into().unwrap()) as u64;
        let comment = u16::from_le_bytes(header[28..30].try_into().unwrap()) as u64;
        let mut name = vec![0; name_len];
        source.read_exact(&mut name).map_err(|e| e.to_string())?;
        let name = std::str::from_utf8(&name).map_err(|_| "backup_entry_path_invalid")?;
        entry_name(name)?;
        if !central_names.insert(name.to_ascii_lowercase()) {
            return Err("backup_duplicate_entry".into());
        }
        source
            .seek(SeekFrom::Current((extra + comment) as i64))
            .map_err(|e| e.to_string())?;
        count += 1;
    }
    if count != archive.len() {
        return Err("backup_entry_count_mismatch".into());
    }
    let mut names = BTreeMap::new();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(|e| e.to_string())?;
        entry_name(entry.name())?;
        if entry.name_raw() != entry.name().as_bytes()
            || entry.is_dir()
            || entry.is_symlink()
            || entry.encrypted()
        {
            return Err("backup_entry_type_not_supported".into());
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err("backup_compression_not_supported".into());
        }
        if names.insert(entry.name().to_owned(), index).is_some() {
            return Err("backup_duplicate_entry".into());
        }
    }
    Ok(names)
}

pub(super) fn manifest(
    manifest: &Manifest,
    names: &BTreeMap<String, usize>,
    archive: &mut ZipArchive<File>,
) -> Result<u64, String> {
    if manifest.format != "prompt-shelf-backup" || manifest.version != 1 {
        return Err("backup_format_not_supported".into());
    }
    if !(1..=SCHEMA_VERSION).contains(&manifest.schema_version) {
        return Err("backup_future_or_unknown_schema".into());
    }
    if manifest.app_version.is_empty() || manifest.created_at <= 0 {
        return Err("backup_metadata_invalid".into());
    }
    let mut total = 0u64;
    let mut seen = HashSet::new();
    for record in &manifest.files {
        entry_name(&record.path)?;
        if record.path == "manifest.json"
            || !seen.insert(&record.path)
            || record.bytes == 0
            || record.sha256.len() != 64
            || !record
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("backup_file_record_invalid".into());
        }
        let index = *names.get(&record.path).ok_or("backup_file_missing")?;
        if archive
            .by_index_raw(index)
            .map_err(|e| e.to_string())?
            .size()
            != record.bytes
        {
            return Err("backup_declared_size_mismatch".into());
        }
        total = total
            .checked_add(record.bytes)
            .ok_or("backup_size_overflow")?;
    }
    if !seen.contains(&"library.db".to_owned()) || names.len() != seen.len() + 1 {
        return Err("backup_file_set_mismatch".into());
    }
    Ok(total)
}

fn schema(c: &Connection) -> Result<Vec<(String, String, String, String)>, String> {
    let mut statement=c.prepare("SELECT type,name,tbl_name,coalesce(sql,'') FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").map_err(sql)?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, String>(3)?
                    .replace("\r\n", "\n")
                    .trim()
                    .to_owned(),
            ))
        })
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    Ok(rows)
}

pub(super) fn database(
    root: &Path,
    manifest: &Manifest,
    control: &JobControl,
) -> Result<(), String> {
    let path = root.join("library.db");
    let mut c =
        Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(sql)?;
    c.pragma_update(None, "trusted_schema", false)
        .map_err(sql)?;
    let version: i64 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(sql)?;
    if version != manifest.schema_version {
        return Err("backup_schema_manifest_mismatch".into());
    }
    let expected = Connection::open_in_memory().map_err(sql)?;
    expected
        .execute_batch(include_str!("../migrations/001_initial.sql"))
        .map_err(sql)?;
    if version == 2 {
        expected
            .execute_batch(include_str!("../migrations/002_drafts.sql"))
            .map_err(sql)?;
    }
    if schema(&c)? != schema(&expected)? {
        return Err("backup_database_structure_invalid".into());
    }
    // Only after exact application-owned schema is verified may application migration SQL run.
    if version == 1 {
        let tx = c.transaction().map_err(sql)?;
        tx.execute_batch(include_str!("../migrations/002_drafts.sql"))
            .map_err(sql)?;
        tx.pragma_update(None, "user_version", 2).map_err(sql)?;
        tx.commit().map_err(sql)?;
    }
    c.pragma_update(None, "foreign_keys", true).map_err(sql)?;
    let integrity: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(sql)?;
    if integrity != "ok" {
        return Err(format!("backup_integrity_failed: {integrity}"));
    }
    let mut foreign = c.prepare("PRAGMA foreign_key_check").map_err(sql)?;
    if foreign
        .query([])
        .map_err(sql)?
        .next()
        .map_err(sql)?
        .is_some()
    {
        return Err("backup_foreign_keys_failed".into());
    }
    drop(foreign);
    if summary_from(&c)? != manifest.summary {
        return Err("backup_summary_mismatch".into());
    }
    catalog::preferences_from(&c)?;
    catalog::dictionary_from(&c)?;
    let bad_settings:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key NOT IN('ui_preferences','dictionary_initialized') OR (key='dictionary_initialized' AND value_json!='true'))",[],|r|r.get(0)).map_err(sql)?;
    if bad_settings {
        return Err("backup_settings_not_supported".into());
    }
    let orphans:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM assets a WHERE NOT EXISTS(SELECT 1 FROM case_assets ca WHERE ca.asset_id=a.id) AND NOT EXISTS(SELECT 1 FROM draft_assets da WHERE da.asset_id=a.id))",[],|r|r.get(0)).map_err(sql)?;
    if orphans {
        return Err("backup_unreferenced_assets".into());
    }
    let files = manifest
        .files
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let mut q=c.prepare("SELECT sha256,extension,mime,byte_size,width,height,relative_path FROM assets ORDER BY relative_path").map_err(sql)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3).and_then(|value| {
                    u64::try_from(value)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, value))
                })?,
                r.get::<_, u32>(4)?,
                r.get::<_, u32>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    drop(q);
    if rows.len() + 1 != files.len() {
        return Err("backup_asset_set_mismatch".into());
    }
    for (hash, extension, mime, bytes, width, height, relative) in rows {
        control.check()?;
        if relative != DataPaths::asset_relative(&hash, &extension)? {
            return Err("backup_asset_path_mismatch".into());
        }
        let record = files.get(relative.as_str()).ok_or("backup_asset_missing")?;
        if record.sha256 != hash || record.bytes != bytes {
            return Err("backup_asset_metadata_mismatch".into());
        }
        image(&root.join(&relative), &extension, &mime, width, height)?;
    }
    c.pragma_update(None, "journal_mode", "DELETE")
        .map_err(sql)?;
    drop(c);
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn image(path: &Path, extension: &str, mime: &str, width: u32, height: u32) -> Result<(), String> {
    use image::{ImageDecoder, ImageFormat, ImageReader};
    let mut reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let format = reader.format().ok_or("backup_image_format_invalid")?;
    let expected = match format {
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        ImageFormat::WebP => ("webp", "image/webp"),
        _ => return Err("backup_image_format_invalid".into()),
    };
    if expected != (extension, mime) {
        return Err("backup_image_format_mismatch".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut pixels = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    pixels.apply_orientation(orientation);
    if (pixels.width(), pixels.height()) != (width, height) {
        return Err("backup_image_dimensions_mismatch".into());
    }
    Ok(())
}
