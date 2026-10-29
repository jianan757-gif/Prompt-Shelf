//! Startup-only clipboard protection. Keep the clipboard locked until candidate cleanup ends.
use crate::{
    gc::{ClipboardProtection, GcReport},
    store::Store,
};

#[cfg(not(windows))]
pub fn startup_gc(store: &Store) -> Result<GcReport, String> {
    // No reliable clipboard reader on this platform yet.
    store.collect_garbage(ClipboardProtection::Unknown)
}

#[cfg(windows)]
pub fn startup_gc(store: &Store) -> Result<GcReport, String> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
    use windows_sys::Win32::{
        Foundation::{GetLastError, SetLastError},
        System::DataExchange::{
            CloseClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
        },
        UI::Shell::DragQueryFileW,
    };
    struct Open;
    impl Drop for Open {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    // This is read-only; failure conservatively retains every candidate.
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return store.collect_garbage(ClipboardProtection::Unknown);
    }
    let _clipboard = Open;
    let read = || -> Option<Vec<PathBuf>> {
        let mut format = 0;
        let mut has_files = false;
        let mut formats = 0;
        loop {
            unsafe {
                SetLastError(0);
            }
            let next = unsafe { EnumClipboardFormats(format) };
            if next == 0 {
                if unsafe { GetLastError() } != 0 {
                    return None;
                }
                break;
            }
            has_files |= next == 15; // CF_HDROP
            format = next;
            formats += 1;
            if formats > 4096 {
                return None;
            }
        }
        if !has_files {
            return Some(vec![]);
        }
        let handle = unsafe { GetClipboardData(15) };
        if handle.is_null() {
            return None;
        }
        let count = unsafe { DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0) };
        if count == 0 || count > 4096 {
            return None;
        }
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let length = unsafe { DragQueryFileW(handle, index, std::ptr::null_mut(), 0) };
            if length == 0 || length > 32768 {
                return None;
            }
            let mut value = vec![0u16; length as usize + 1];
            if unsafe { DragQueryFileW(handle, index, value.as_mut_ptr(), length + 1) } != length {
                return None;
            }
            let path = PathBuf::from(OsString::from_wide(&value[..length as usize]));
            if !path.is_absolute() {
                return None;
            }
            paths.push(path);
        }
        Some(paths)
    };
    let protection = read()
        .map(ClipboardProtection::Files)
        .unwrap_or(ClipboardProtection::Unknown);
    // _clipboard deliberately remains alive for this entire call: another process cannot
    // replace CF_HDROP between inspection and unlink.
    store.collect_garbage(protection)
}
