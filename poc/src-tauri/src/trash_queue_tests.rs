//! Real queue with immutable Store output snapshots; native input is replaced by a file-reading fake.
use super::*;
use prompt_shelf_poc::{
    cases::{EditSessions, ImageReference},
    gc::ClipboardProtection,
};
use std::sync::atomic::{AtomicBool, AtomicUsize};

#[derive(Default)]
struct FilePlatform {
    prepared: AtomicUsize,
    sent: AtomicUsize,
}
impl PastePlatform for FilePlatform {
    type Target = ();
    type Prepared = PasteItem;
    fn capture_target(&self) -> Result<(), String> {
        Ok(())
    }
    fn target_is_foreground(&self, _: &()) -> Result<(), String> {
        Ok(())
    }
    fn request_restore(&self, _: &()) -> Result<(), String> {
        Ok(())
    }
    fn restoration_status(&self, _: &()) -> Result<bool, String> {
        Ok(true)
    }
    fn may_begin(&self, _: &()) -> Result<(), String> {
        Ok(())
    }
    fn prepare(&self, item: PasteItem) -> Result<PasteItem, String> {
        if let PasteItem::Image(path, _) = &item {
            image::open(path).map_err(|e| e.to_string())?;
        }
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(item)
    }
    fn write_clipboard(&self, item: PasteItem) -> Result<(), String> {
        if let PasteItem::Image(path, _) = item {
            std::fs::read(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn dispatch(&self, _: &()) -> Result<(), String> {
        self.sent.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn deleting_source_during_real_queue_preserves_snapshot_files_after_completion_and_cancel() {
    for cancel in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let store = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
        let mut document = store.edit_document(None).unwrap();
        document.title = "output lease".into();
        document.prompt = "unchanged".into();
        for color in [40, 80] {
            let path = source.path().join(format!("{color}.png"));
            image::RgbaImage::from_pixel(10, 8, image::Rgba([color, 90, 160, 255]))
                .save(&path)
                .unwrap();
            let a = store.import_image(&path).unwrap();
            document.references.push(ImageReference {
                id: uuid::Uuid::new_v4().to_string(),
                asset_id: a.id,
                original_name: "ref.png".into(),
            });
        }
        store.commit_case(&document).unwrap();
        drop(store); // Remove import pins, so only output pins can protect.
        let store = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
        let library = store.output_snapshot_at_revision(&document.id, 1).unwrap();
        let items = paste_items(&library, &document.id, "queue", None).unwrap();
        let platform = Arc::new(FilePlatform::default());
        let queue = queue::Queue::default();
        let ticket = queue.reserve().unwrap();
        let request = ticket.id;
        let deleted = AtomicBool::new(false);
        let terminal = Mutex::new(String::new());
        queue::run(
            platform.clone(),
            (),
            ticket,
            document.id.clone(),
            items,
            Duration::from_millis(1),
            |event| {
                if event.done == 1 && !deleted.swap(true, Ordering::SeqCst) {
                    store.soft_delete_case(&document.id).unwrap();
                    store.permanently_delete_case(&document.id).unwrap();
                    let gc = store
                        .collect_garbage(ClipboardProtection::Files(vec![]))
                        .unwrap();
                    assert_eq!(gc.retained, 2);
                    assert_eq!(gc.removed, 0);
                    if cancel {
                        queue.cancel_request(request);
                    }
                }
                if ["completed", "cancelled", "failed"].contains(&event.phase.as_str()) {
                    *terminal.lock().unwrap() = event.phase;
                }
            },
        )
        .await;
        assert_eq!(
            &*terminal.lock().unwrap(),
            if cancel { "cancelled" } else { "completed" }
        );
        assert_eq!(
            platform.sent.load(Ordering::SeqCst),
            if cancel { 1 } else { 2 }
        );
        assert_eq!(
            store
                .collect_garbage(ClipboardProtection::Files(vec![]))
                .unwrap()
                .retained,
            2
        );
        for asset in &library.assets {
            assert!(PathBuf::from(&asset.original).is_file());
        }
        drop(store);
        let store = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
        assert_eq!(
            store
                .collect_garbage(ClipboardProtection::Files(vec![]))
                .unwrap()
                .removed,
            2
        );
    }
}

#[test]
fn trash_gate_rejects_active_or_late_editor_and_deleted_cases_cannot_start_new_output() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(DataPaths::new(root.path()).unwrap()).unwrap();
    let mut document = store.edit_document(None).unwrap();
    document.title = "case".into();
    document.prompt = "p".into();
    store.commit_case(&document).unwrap();
    let edits = EditSessions::default();
    let receipt = edits
        .begin_persistent(&store, Some(&document.id), None, false, edits.epoch())
        .unwrap();
    let epoch = edits.epoch();
    assert!(edits
        .while_idle(|| store.soft_delete_case(&document.id))
        .is_err());
    edits.discard(&store, &receipt.session_id).unwrap();
    edits
        .while_idle(|| store.soft_delete_case(&document.id))
        .unwrap();
    assert!(edits
        .stash(&store, &receipt.session_id, &receipt.document, 99)
        .is_err());
    assert!(edits
        .begin_persistent(&store, None, None, false, epoch)
        .is_err());
    assert!(store.output_snapshot_at_revision(&document.id, 1).is_err());
}
