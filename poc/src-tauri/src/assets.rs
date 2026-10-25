use crate::paths::DataPaths;
use image::{ImageDecoder, ImageEncoder, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub struct PreparedAsset {
    pub hash: String,
    pub extension: &'static str,
    pub mime: &'static str,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub relative: String,
}
pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn decode(path: &Path) -> Result<(image::DynamicImage, Option<Vec<u8>>), String> {
    let mut reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("image_decode: {e}"))?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let icc = decoder.icc_profile().map_err(|e| e.to_string())?;
    let mut pixels = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    pixels.apply_orientation(orientation);
    Ok((pixels, icc))
}
/// Copy and hash into same-volume staging; decode that stable copy, never rewrite source.
pub fn prepare(
    paths: &DataPaths,
    source: &Path,
    expected_hash: &str,
) -> Result<PreparedAsset, String> {
    let mut source = File::open(source).map_err(|e| format!("image_open: {e}"))?;
    let mut staged = tempfile::NamedTempFile::new_in(paths.staging()).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut bytes = 0u64;
    loop {
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > 512 * 1024 * 1024 {
            return Err("image_file_exceeds_512_mib".into());
        }
        staged.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buffer[..n]);
    }
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    let hash = format!("{:x}", hasher.finalize());
    if hash_file(staged.path())? != hash {
        return Err("staging_hash_mismatch".into());
    }
    if !expected_hash.is_empty() && hash != expected_hash.to_lowercase() {
        return Err("asset_hash_mismatch".into());
    }
    let format = ImageReader::open(staged.path())
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .format()
        .ok_or("unsupported_image_format")?;
    let (extension, mime) = match format {
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        ImageFormat::WebP => ("webp", "image/webp"),
        _ => return Err("unsupported_image_format".into()),
    };
    let (pixels, icc) = decode(staged.path())?;
    let relative = DataPaths::asset_relative(&hash, extension)?;
    let destination = paths.destination(&relative);
    if destination.exists() {
        let real = paths.resolve(&relative, "assets")?;
        if hash_file(&real)? != hash {
            return Err("immutable_asset_hash_mismatch".into());
        }
    } else {
        // No replacement, including a racing writer. Same data is revalidated below.
        match staged.persist_noclobber(&destination) {
            Ok(file) => file.sync_all().map_err(|e| e.to_string())?,
            Err(error) if destination.exists() => {
                if hash_file(&paths.resolve(&relative, "assets")?)? != hash {
                    return Err("immutable_asset_hash_mismatch".into());
                }
                drop(error);
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    write_thumbnail(paths, &hash, &pixels, icc)?;
    Ok(PreparedAsset {
        hash,
        extension,
        mime,
        bytes,
        width: pixels.width(),
        height: pixels.height(),
        relative,
    })
}
fn write_thumbnail(
    paths: &DataPaths,
    hash: &str,
    pixels: &image::DynamicImage,
    icc: Option<Vec<u8>>,
) -> Result<PathBuf, String> {
    let relative = DataPaths::thumb_relative(hash)?;
    let destination = paths.destination(&relative);
    let replacing = destination.exists();
    if replacing {
        let existing = paths.resolve(&relative, "cache/thumbs")?;
        if valid_thumbnail(&existing) {
            return Ok(existing);
        }
    }
    // 180px high for list DPI, preserve ratio and alpha; never upscale a small source.
    let height = pixels.height().min(180);
    let width = ((pixels.width() as u64 * height as u64) / pixels.height() as u64).max(1) as u32;
    let thumb = pixels
        .resize_exact(width, height, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut staged = tempfile::NamedTempFile::new_in(paths.staging()).map_err(|e| e.to_string())?;
    let mut encoder = image::codecs::png::PngEncoder::new(staged.as_file_mut());
    if let Some(profile) = icc {
        encoder
            .set_icc_profile(profile)
            .map_err(|e| e.to_string())?;
    }
    encoder
        .write_image(
            thumb.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    if replacing {
        staged
            .persist(&destination)
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;
    } else {
        match staged.persist_noclobber(&destination) {
            Ok(file) => file.sync_all().map_err(|e| e.to_string())?,
            Err(error) if destination.exists() => drop(error),
            Err(error) => return Err(error.to_string()),
        }
    }
    paths.resolve(&relative, "cache/thumbs")
}
fn valid_thumbnail(path: &Path) -> bool {
    let Ok(reader) = ImageReader::open(path) else {
        return false;
    };
    let Ok(mut reader) = reader.with_guessed_format() else {
        return false;
    };
    if reader.format() != Some(ImageFormat::Png) {
        return false;
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(180);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().is_ok()
}
pub fn thumbnail(paths: &DataPaths, hash: &str, original: &Path) -> Result<PathBuf, String> {
    let relative = DataPaths::thumb_relative(hash)?;
    if paths.destination(&relative).exists() {
        let existing = paths.resolve(&relative, "cache/thumbs")?;
        if valid_thumbnail(&existing) {
            return Ok(existing);
        }
    }
    if hash_file(original)? != hash {
        return Err("immutable_asset_hash_mismatch".into());
    }
    let (pixels, icc) = decode(original)?;
    write_thumbnail(paths, hash, &pixels, icc)
}
