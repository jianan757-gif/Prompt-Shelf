use super::{PasteItem, PastePlatform};
#[derive(Default)]
pub struct NativePlatform;
impl PastePlatform for NativePlatform {
    type Target = ();
    type Prepared = ();
    fn capture_target(&self) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
    fn target_is_foreground(&self, _: &()) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
    fn request_restore(&self, _: &()) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
    fn restoration_status(&self, _: &()) -> Result<bool, String> {
        Err("non_windows_not_implemented".into())
    }
    fn may_begin(&self, _: &()) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
    fn prepare(&self, _: PasteItem) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
    fn write_clipboard(&self, _: ()) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
    fn dispatch(&self, _: &()) -> Result<(), String> {
        Err("non_windows_not_implemented".into())
    }
}
