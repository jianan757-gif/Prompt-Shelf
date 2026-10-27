//! Clipboard import only. Existing clipboard output/focus/SendInput modules are untouched.
use std::path::PathBuf;
pub enum ClipboardImage {
    Files(Vec<PathBuf>),
    Png(Vec<u8>),
    Dib(Vec<u8>),
    Empty,
}

pub fn png_payload(raw: &[u8]) -> Result<Vec<u8>, String> {
    if !raw.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("clipboard_png_invalid".into());
    }
    let mut offset = 8usize;
    while offset.checked_add(12).is_some_and(|end| end <= raw.len()) {
        let length = u32::from_be_bytes(raw[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(length)
            .and_then(|n| n.checked_add(12))
            .ok_or("clipboard_png_overflow")?;
        if end > raw.len() {
            return Err("clipboard_png_truncated".into());
        }
        if &raw[offset + 4..offset + 8] == b"IEND" {
            return Ok(raw[..end].to_vec());
        }
        offset = end;
    }
    Err("clipboard_png_missing_iend".into())
}
pub fn dib_png(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.len() < 40 {
        return Err("clipboard_dib_truncated".into());
    }
    let read = |index| u32::from_le_bytes(raw[index..index + 4].try_into().unwrap());
    let header = read(0) as usize;
    if ![40, 52, 56, 108, 124].contains(&header) || raw.len() < header {
        return Err("clipboard_dib_header_unsupported".into());
    }
    let bits = u16::from_le_bytes(raw[14..16].try_into().unwrap());
    let compression = read(16);
    let palette = if bits <= 8 {
        let used = read(32);
        if used == 0 {
            1usize << bits
        } else {
            used as usize
        }
    } else {
        0
    };
    let masks = if header == 40 {
        match compression {
            3 => 12,
            6 => 16,
            _ => 0,
        }
    } else {
        0
    };
    let offset = 14usize
        .checked_add(header)
        .and_then(|n| n.checked_add(masks))
        .and_then(|n| n.checked_add(palette.checked_mul(4)?))
        .ok_or("clipboard_dib_overflow")?;
    if offset >= raw.len() + 14 {
        return Err("clipboard_dib_missing_pixels".into());
    }
    let mut bmp = b"BM".to_vec();
    bmp.extend_from_slice(
        &u32::try_from(raw.len() + 14)
            .map_err(|_| "clipboard_dib_too_large")?
            .to_le_bytes(),
    );
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&(offset as u32).to_le_bytes());
    bmp.extend_from_slice(raw);
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bmp), image::ImageFormat::Bmp);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let pixels = reader
        .decode()
        .map_err(|e| format!("clipboard_dib_decode: {e}"))?;
    let mut png = std::io::Cursor::new(vec![]);
    pixels
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png.into_inner())
}
#[cfg(not(windows))]
pub fn read() -> Result<ClipboardImage, String> {
    Err("clipboard_image_import_not_implemented_on_this_platform".into())
}
#[cfg(windows)]
pub fn read() -> Result<ClipboardImage, String> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::{
        System::{DataExchange::*, Memory::*},
        UI::Shell::DragQueryFileW,
    };
    struct Clipboard;
    impl Drop for Clipboard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    unsafe fn bytes(format: u32) -> Result<Vec<u8>, String> {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return Err("clipboard_data_unavailable".into());
        }
        let length = GlobalSize(handle);
        if length == 0 || length > 512 * 1024 * 1024 {
            return Err("clipboard_data_size_invalid".into());
        }
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            return Err("clipboard_data_lock_failed".into());
        }
        let output = std::slice::from_raw_parts(ptr as *const u8, length).to_vec();
        GlobalUnlock(handle);
        Ok(output)
    }
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("clipboard_busy_retry".into());
        }
        let _clipboard = Clipboard;
        if IsClipboardFormatAvailable(15) != 0 {
            let handle = GetClipboardData(15);
            if !handle.is_null() {
                let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0);
                if count > 500 {
                    return Err("clipboard_too_many_files".into());
                }
                let mut files = vec![];
                for index in 0..count {
                    let length = DragQueryFileW(handle, index, std::ptr::null_mut(), 0);
                    if length > 32767 {
                        return Err("clipboard_path_too_long".into());
                    }
                    let mut name = vec![0u16; length as usize + 1];
                    DragQueryFileW(handle, index, name.as_mut_ptr(), name.len() as u32);
                    let path =
                        PathBuf::from(std::ffi::OsString::from_wide(&name[..length as usize]));
                    if path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|ext| {
                            ["png", "jpg", "jpeg", "webp"]
                                .contains(&ext.to_ascii_lowercase().as_str())
                        })
                    {
                        files.push(path);
                    }
                }
                if !files.is_empty() {
                    return Ok(ClipboardImage::Files(files));
                }
            }
        }
        let name: Vec<u16> = std::ffi::OsStr::new("PNG")
            .encode_wide()
            .chain(Some(0))
            .collect();
        let png = RegisterClipboardFormatW(name.as_ptr());
        if png != 0 && IsClipboardFormatAvailable(png) != 0 {
            return Ok(ClipboardImage::Png(png_payload(&bytes(png)?)?));
        }
        for format in [17, 8] {
            if IsClipboardFormatAvailable(format) != 0 {
                return Ok(ClipboardImage::Dib(bytes(format)?));
            }
        }
        Ok(ClipboardImage::Empty)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_png_padding_is_not_saved_as_image_data() {
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 96]));
        let mut out = std::io::Cursor::new(vec![]);
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        let expected = out.into_inner();
        let mut padded = expected.clone();
        padded.extend_from_slice(&[0; 16]);
        assert_eq!(png_payload(&padded).unwrap(), expected);
        assert!(png_payload(b"bad").is_err());
    }
    #[test]
    fn clipboard_dib_converts_without_clipboard_or_system_input() {
        let mut dib = vec![0u8; 40];
        dib[..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&2i32.to_le_bytes());
        dib[8..12].copy_from_slice(&1i32.to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[10, 20, 30, 40, 50, 60, 0, 0]);
        let png = dib_png(&dib).unwrap();
        let pixels = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(pixels.get_pixel(0, 0).0, [30, 20, 10]);
        assert_eq!(pixels.get_pixel(1, 0).0, [60, 50, 40]);
        assert!(dib_png(&dib[..20]).is_err());
    }
}
