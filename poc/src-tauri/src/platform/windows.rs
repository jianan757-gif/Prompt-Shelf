use super::{PasteItem, PastePlatform};
use image::ImageEncoder;
use std::{
    io::Cursor,
    os::windows::ffi::OsStrExt,
    sync::atomic::{AtomicIsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{DataExchange::*, Memory::*, Threading::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub struct NativePlatform {
    owner: AtomicIsize,
}
impl Default for NativePlatform {
    fn default() -> Self {
        Self {
            owner: AtomicIsize::new(0),
        }
    }
}
impl NativePlatform {
    pub fn attach_window(&self, window: &tauri::WebviewWindow) -> Result<(), tauri::Error> {
        self.owner
            .store(window.hwnd()?.0 as isize, Ordering::Release);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Target {
    window: isize,
    pid: u32,
    thread: u32,
    process_created: u64,
    pub captured_ms: u128,
}
pub struct Prepared {
    formats: Vec<(u32, Vec<u8>)>,
}
#[derive(Debug, PartialEq)]
enum CaptureClass {
    External,
    QuickPanel,
    OtherOwn,
}
fn classify_capture(window: isize, pid: u32, quick: isize, ours: u32) -> CaptureClass {
    if pid != ours {
        CaptureClass::External
    } else if window != 0 && window == quick {
        CaptureClass::QuickPanel
    } else {
        CaptureClass::OtherOwn
    }
}
fn permitted_transition(current: isize, target: isize, quick: isize) -> bool {
    current != 0 && (current == target || (quick != 0 && current == quick))
}
fn restoration_observation(current: isize, target: isize, quick: isize) -> Result<bool, String> {
    if current == target && target != 0 {
        Ok(true)
    } else if current == 0 || (quick != 0 && current == quick) {
        Ok(false)
    } else {
        Err("foreground_changed_during_restore".into())
    }
}

fn identity(window: HWND) -> Result<(u32, u32, u64), String> {
    unsafe {
        if window.is_null() || IsWindow(window) == 0 {
            return Err("target_closed".into());
        }
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(window, &mut pid);
        if thread == 0 || pid == 0 {
            return Err("target_identity_unavailable".into());
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err("target_process_access_denied".into());
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exit = created;
        let mut kernel = created;
        let mut user = created;
        let ok = GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user);
        CloseHandle(process);
        if ok == 0 {
            return Err("target_process_identity_failed".into());
        }
        Ok((
            pid,
            thread,
            ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
        ))
    }
}
impl Target {
    fn verify(&self) -> Result<(), String> {
        let now = identity(self.window as HWND)?;
        if now != (self.pid, self.thread, self.process_created) {
            return Err("target_identity_changed".into());
        }
        Ok(())
    }
}

impl PastePlatform for NativePlatform {
    type Target = Target;
    type Prepared = Prepared;
    fn target_diagnostic(&self, target: &Target) -> serde_json::Value {
        serde_json::json!({"hwnd":format!("0x{:x}",target.window),"pid":target.pid,"threadId":target.thread,"processCreated":target.process_created,"capturedMs":target.captured_ms})
    }
    fn capture_target(&self) -> Result<Target, String> {
        unsafe {
            let window = GetForegroundWindow();
            let (pid, thread, process_created) = identity(window)?;
            match classify_capture(
                window as isize,
                pid,
                self.owner.load(Ordering::Acquire),
                std::process::id(),
            ) {
                CaptureClass::QuickPanel => return Err("quick_panel_keep_previous_target".into()),
                CaptureClass::OtherOwn => {
                    return Err("other_own_window_rejected_clear_target".into())
                }
                CaptureClass::External => {}
            }
            Ok(Target {
                window: window as isize,
                pid,
                thread,
                process_created,
                captured_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
            })
        }
    }
    fn target_is_foreground(&self, target: &Target) -> Result<(), String> {
        target.verify()?;
        let actual = unsafe { GetForegroundWindow() } as isize;
        if actual != target.window {
            return Err(if actual == self.owner.load(Ordering::Acquire) {
                "panel_interaction_stop"
            } else {
                "foreground_changed_stop"
            }
            .into());
        }
        Ok(())
    }
    fn may_begin(&self, target: &Target) -> Result<(), String> {
        target.verify()?;
        let current = unsafe { GetForegroundWindow() };
        if !permitted_transition(
            current as isize,
            target.window,
            self.owner.load(Ordering::Acquire),
        ) {
            return Err("foreground_changed_before_start".into());
        }
        Ok(())
    }
    fn panel_is_foreground(&self) -> bool {
        let owner = self.owner.load(Ordering::Acquire);
        owner != 0 && unsafe { GetForegroundWindow() } as isize == owner
    }
    fn request_restore(&self, target: &Target) -> Result<(), String> {
        // Request while our ordinary editable panel owns foreground. It stays visible.
        // Never retry SetForegroundWindow or activate over a third party.
        self.may_begin(target)?;
        unsafe {
            if GetForegroundWindow() as isize != target.window
                && SetForegroundWindow(target.window as HWND) == 0
            {
                return Err("set_foreground_denied_no_paste".into());
            }
        }
        Ok(())
    }
    fn restoration_status(&self, target: &Target) -> Result<bool, String> {
        target.verify()?;
        let current = unsafe { GetForegroundWindow() } as isize;
        restoration_observation(current, target.window, self.owner.load(Ordering::Acquire))
    }
    fn foreground_diagnostic(&self) -> serde_json::Value {
        unsafe {
            let current = GetForegroundWindow();
            let mut pid = 0;
            let thread = if current.is_null() {
                0
            } else {
                GetWindowThreadProcessId(current, &mut pid)
            };
            let panel = self.owner.load(Ordering::Acquire) as HWND;
            serde_json::json!({"actualHwnd":format!("0x{:x}",current as isize),"actualPid":pid,"actualThreadId":thread,"panelHwnd":format!("0x{:x}",panel as isize),"panelVisible":!panel.is_null()&&IsWindowVisible(panel)!=0})
        }
    }
    fn prepare(&self, item: PasteItem) -> Result<Prepared, String> {
        match item {
            PasteItem::Text(text) => {
                if text.contains('\0') {
                    return Err("text_contains_nul".into());
                }
                Ok(Prepared {
                    formats: vec![(
                        13,
                        text.encode_utf16()
                            .chain([0])
                            .flat_map(u16::to_le_bytes)
                            .collect(),
                    )],
                })
            }
            PasteItem::Image(path, _) => {
                let bytes = std::fs::read(&path).map_err(|e| format!("image_read: {e}"))?;
                let decoded = image::load_from_memory(&bytes)
                    .map_err(|e| format!("image_decode: {e}"))?
                    .to_rgba8();
                let (width, height) = decoded.dimensions();
                let png = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                    bytes
                } else {
                    let mut png = Cursor::new(Vec::new());
                    image::codecs::png::PngEncoder::new(&mut png)
                        .write_image(&decoded, width, height, image::ExtendedColorType::Rgba8)
                        .map_err(|e| e.to_string())?;
                    png.into_inner()
                };
                let png_id = unsafe { RegisterClipboardFormatW(windows_sys::core::w!("PNG")) };
                if png_id == 0 {
                    return Err("png_format_registration_failed".into());
                }
                // Compatibility representations only. Original bytes on disk stay untouched.
                let dib_bytes = dib(&decoded, false)?;
                let dibv5 = dib(&decoded, true)?;
                let mut drop = vec![0u8; 20];
                drop[..4].copy_from_slice(&20u32.to_le_bytes());
                drop[16..20].copy_from_slice(&1u32.to_le_bytes());
                for word in path.as_os_str().encode_wide().chain([0, 0]) {
                    drop.extend(word.to_le_bytes());
                }
                Ok(Prepared {
                    formats: vec![(png_id, png), (17, dibv5), (8, dib_bytes), (15, drop)],
                })
            }
        }
    }
    fn write_clipboard(&self, prepared: Prepared) -> Result<(), String> {
        // Allocate everything before EmptyClipboard. On SetClipboardData failure,
        // clear partial formats while still holding this single open cycle.
        let mut handles = Vec::new();
        for (format, bytes) in &prepared.formats {
            unsafe {
                let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
                if handle.is_null() {
                    free_handles(&handles);
                    return Err("clipboard_allocation_failed".into());
                }
                let dest = GlobalLock(handle);
                if dest.is_null() {
                    GlobalFree(handle);
                    free_handles(&handles);
                    return Err("clipboard_lock_failed".into());
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest as *mut u8, bytes.len());
                GlobalUnlock(handle);
                handles.push((*format, handle));
            }
        }
        unsafe {
            let owner = self.owner.load(Ordering::Acquire) as HWND;
            if owner.is_null() || OpenClipboard(owner) == 0 {
                free_handles(&handles);
                return Err("clipboard_busy_or_no_owner".into());
            }
            let _close = ClipboardClose;
            if EmptyClipboard() == 0 {
                free_handles(&handles);
                return Err("clipboard_empty_failed".into());
            }
            for i in 0..handles.len() {
                if SetClipboardData(handles[i].0, handles[i].1).is_null() {
                    EmptyClipboard();
                    free_handles(&handles[i..]);
                    return Err("clipboard_format_commit_failed_no_paste".into());
                }
            }
        }
        Ok(())
    }
    fn dispatch(&self, target: &Target) -> Result<(), String> {
        self.target_is_foreground(target)?;
        unsafe {
            for key in [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN] {
                if GetAsyncKeyState(key as i32) < 0 {
                    return Err("release_modifier_keys_before_paste".into());
                }
            }
            let key = |vk, flags| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let inputs = [
                key(VK_CONTROL, 0),
                key(0x56, 0),
                key(0x56, KEYEVENTF_KEYUP),
                key(VK_CONTROL, KEYEVENTF_KEYUP),
            ];
            // Last possible identity / foreground check before the single SendInput call.
            self.target_is_foreground(target)?;
            if SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            ) != inputs.len() as u32
            {
                // Release any modifiers if Windows accepted a partial batch.
                let release = [key(0x56, KEYEVENTF_KEYUP), key(VK_CONTROL, KEYEVENTF_KEYUP)];
                SendInput(2, release.as_ptr(), std::mem::size_of::<INPUT>() as i32);
                return Err("send_input_failed_or_partial".into());
            }
        }
        Ok(())
    }
}
unsafe fn free_handles(handles: &[(u32, HGLOBAL)]) {
    for (_, handle) in handles {
        GlobalFree(*handle);
    }
}
struct ClipboardClose;
impl Drop for ClipboardClose {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

fn dib(rgba: &image::RgbaImage, v5: bool) -> Result<Vec<u8>, String> {
    let (w, h) = rgba.dimensions();
    let pixels = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or("image_dimensions_overflow")?;
    if w > i32::MAX as u32 || h > i32::MAX as u32 {
        return Err("image_dimensions_overflow".into());
    }
    let size = if v5 { 124 } else { 40 };
    let mut out = vec![0u8; size];
    let set = |out: &mut Vec<u8>, offset, value: u32| {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
    };
    set(&mut out, 0, size as u32);
    set(&mut out, 4, w);
    set(&mut out, 8, h);
    out[12..14].copy_from_slice(&1u16.to_le_bytes());
    out[14..16].copy_from_slice(&32u16.to_le_bytes());
    set(&mut out, 16, if v5 { 3 } else { 0 });
    set(&mut out, 20, pixels);
    if v5 {
        for (offset, mask) in [
            (40, 0x00ff0000),
            (44, 0x0000ff00),
            (48, 0x000000ff),
            (52, 0xff000000),
            (56, 0x73524742),
            (108, 4),
        ] {
            set(&mut out, offset, mask);
        }
    }
    out.reserve(pixels as usize);
    for y in (0..h).rev() {
        for x in 0..w {
            let p = rgba.get_pixel(x, y).0;
            out.extend([p[2], p[1], p[0], if v5 { p[3] } else { 255 }]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_attached_quick_panel_preserves_target() {
        assert_eq!(classify_capture(10, 1, 10, 1), CaptureClass::QuickPanel);
        assert_eq!(classify_capture(20, 1, 10, 1), CaptureClass::OtherOwn);
        assert_eq!(classify_capture(10, 2, 10, 1), CaptureClass::External);
    }
    #[test]
    fn only_exact_quick_or_external_target_can_transition() {
        assert!(permitted_transition(10, 30, 10));
        assert!(permitted_transition(30, 30, 10));
        assert!(!permitted_transition(20, 30, 10)); // library / tray / third party
        assert!(!permitted_transition(0, 30, 10));
        assert!(!permitted_transition(0, 30, 0));
    }
    #[test]
    fn activation_pending_accepts_only_panel_null_or_original_target() {
        assert_eq!(restoration_observation(30, 30, 10), Ok(true));
        assert_eq!(restoration_observation(10, 30, 10), Ok(false));
        assert_eq!(restoration_observation(0, 30, 10), Ok(false));
        assert!(restoration_observation(20, 30, 10).is_err());
    }
    #[test]
    fn dib_headers_alpha_and_bottom_up() {
        let im = image::RgbaImage::from_raw(1, 2, vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let v5 = dib(&im, true).unwrap();
        assert_eq!(&v5[124..], &[7, 6, 5, 8, 3, 2, 1, 4]);
        assert_eq!(&v5[52..56], &0xff000000u32.to_le_bytes());
        assert_eq!(
            &dib(&im, false).unwrap()[40..],
            &[7, 6, 5, 255, 3, 2, 1, 255]
        );
    }
    #[test]
    fn text_is_full_utf16_with_terminator() {
        let p = NativePlatform::default()
            .prepare(PasteItem::Text("中文\nfull 🐈".into()))
            .unwrap();
        assert_eq!(p.formats[0].0, 13);
        assert_eq!(
            p.formats[0].1,
            "中文\nfull 🐈\0"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn png_original_bytes_and_persistent_hdrop() {
        let dir = std::env::temp_dir().join(format!("prompt-shelf-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("透明原图.png");
        image::RgbaImage::from_pixel(2, 3, image::Rgba([22, 33, 44, 55]))
            .save(&path)
            .unwrap();
        let original = std::fs::read(&path).unwrap();
        let prepared = NativePlatform::default()
            .prepare(PasteItem::Image(path.clone(), "test-asset".into()))
            .unwrap();
        assert_eq!(prepared.formats.len(), 4);
        assert_eq!(prepared.formats[0].1, original);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let words = prepared.formats[3].1[20..]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>();
        assert_eq!(
            String::from_utf16(&words[..words.len() - 2]).unwrap(),
            path.to_str().unwrap()
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
