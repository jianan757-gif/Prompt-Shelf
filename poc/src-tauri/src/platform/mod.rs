use std::path::PathBuf;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::NativePlatform;
#[cfg(not(target_os = "windows"))]
mod unsupported;
#[cfg(not(target_os = "windows"))]
pub use unsupported::NativePlatform;

#[derive(Clone)]
pub enum PasteItem {
    Text(String),
    Image(PathBuf, String),
}

/// OS identities and clipboard representations stay inside each adapter.
pub trait PastePlatform: Send + Sync + 'static {
    type Target: Clone + Send + Sync + 'static;
    type Prepared: Send + 'static;
    fn capture_target(&self) -> Result<Self::Target, String>;
    fn target_is_foreground(&self, target: &Self::Target) -> Result<(), String>;
    /// Request activation once; cross-thread activation may complete later.
    fn request_restore(&self, target: &Self::Target) -> Result<(), String>;
    /// Pending may only mean the exact quick panel or transient null foreground.
    fn restoration_status(&self, target: &Self::Target) -> Result<bool, String>;
    fn may_begin(&self, target: &Self::Target) -> Result<(), String>;
    fn panel_is_foreground(&self) -> bool {
        false
    }
    fn prepare(&self, item: PasteItem) -> Result<Self::Prepared, String>;
    fn write_clipboard(&self, prepared: Self::Prepared) -> Result<(), String>;
    fn dispatch(&self, target: &Self::Target) -> Result<(), String>;
    fn target_diagnostic(&self, _target: &Self::Target) -> serde_json::Value {
        serde_json::json!({})
    }
    fn foreground_diagnostic(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}
