use crate::platform::{PasteItem, PastePlatform};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub request_id: u64,
    pub case_id: String,
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub error: Option<String>,
    pub current_asset: Option<String>,
    pub timestamp_ms: u128,
    pub focus: serde_json::Value,
    pub focus_elapsed_ms: Option<f64>,
}
pub struct Cancellation {
    request_id: u64,
    cancelled: AtomicBool,
    active: AtomicBool,
    target_restored: AtomicBool,
    dispatch_gate: Mutex<()>,
}
impl Cancellation {
    pub fn cancel(&self) {
        let _guard = self.dispatch_gate.lock().unwrap();
        self.cancelled.store(true, Ordering::Release);
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    fn dispatch<T>(&self, send: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.run_if_active(send)
    }
    pub fn run_if_active<T>(&self, send: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let _guard = self.dispatch_gate.lock().unwrap();
        if self.is_cancelled() {
            return Err("cancelled".into());
        }
        if !self.active.load(Ordering::Acquire) {
            return Err("request_finished".into());
        }
        send()
    }
    fn finish(&self) {
        let _guard = self.dispatch_gate.lock().unwrap();
        self.active.store(false, Ordering::Release);
    }
}
struct RequestLifetime(Arc<Cancellation>);
impl Drop for RequestLifetime {
    fn drop(&mut self) {
        self.0.finish();
    }
}
pub struct Queue {
    active: Mutex<Option<Arc<Cancellation>>>,
    next: AtomicU64,
}
impl Default for Queue {
    fn default() -> Self {
        Self {
            active: Mutex::new(None),
            next: AtomicU64::new(1),
        }
    }
}
pub struct Ticket {
    pub id: u64,
    pub cancel: Arc<Cancellation>,
}
impl Queue {
    pub fn reserve(&self) -> Result<Ticket, String> {
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return Err("request_already_active".into());
        }
        let cancel = Arc::new(Cancellation {
            request_id: self.next.fetch_add(1, Ordering::Relaxed),
            cancelled: AtomicBool::new(false),
            active: AtomicBool::new(true),
            target_restored: AtomicBool::new(false),
            dispatch_gate: Mutex::new(()),
        });
        *active = Some(cancel.clone());
        Ok(Ticket {
            id: cancel.request_id,
            cancel,
        })
    }
    pub fn cancel(&self) -> bool {
        if let Some(cancel) = self.active.lock().unwrap().as_ref() {
            cancel.cancel();
            true
        } else {
            false
        }
    }
    pub fn release(&self) {
        *self.active.lock().unwrap() = None;
    }
    pub fn cancel_request(&self, id: u64) -> bool {
        if let Some(active) = self.active.lock().unwrap().as_ref() {
            if active.request_id == id {
                active.cancel();
                return true;
            }
        }
        false
    }
    pub fn cancel_panel_refocus(&self) -> bool {
        if let Some(active) = self.active.lock().unwrap().as_ref() {
            if active.target_restored.load(Ordering::Acquire) {
                active.cancel();
                return true;
            }
        }
        false
    }
    pub fn busy(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }
}

/// A single asynchronous worker; first restore once, never steal focus back.
/// Every wait checks cancellation and foreground; preparation is off executor.
pub async fn run<P: PastePlatform>(
    platform: Arc<P>,
    target: P::Target,
    ticket: Ticket,
    case_id: String,
    items: Vec<PasteItem>,
    interval: Duration,
    emit: impl Fn(Progress),
) {
    let _lifetime = RequestLifetime(ticket.cancel.clone());
    let total = items.len();
    let mut done = 0;
    let mut restored = false;
    let current = Mutex::new(None::<String>);
    let focus_started = Mutex::new(None::<Instant>);
    let event = |phase: &str, done: usize, error: Option<String>| {
        emit(Progress {
            request_id: ticket.id,
            case_id: case_id.clone(),
            phase: phase.into(),
            done,
            total,
            error,
            current_asset: current.lock().unwrap().clone(),
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            focus: platform.foreground_diagnostic(),
            focus_elapsed_ms: focus_started
                .lock()
                .unwrap()
                .as_ref()
                .map(|start| start.elapsed().as_secs_f64() * 1000.0),
        })
    };
    let check = |restored: bool| -> Result<(), String> {
        if ticket.cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        if restored {
            platform.target_is_foreground(&target)?;
        } else {
            // While preparing, only the bound target or exact editable panel
            // may be foreground. A third-party switch stops before activation.
            platform.may_begin(&target)?;
        }
        Ok(())
    };
    let result: Result<(),String>=async {
        for item in items {
            *current.lock().unwrap()=match &item{PasteItem::Image(_,id)=>Some(id.clone()),PasteItem::Text(_)=>None};
            check(restored)?;
            event("preparing",done,None);
            let p=platform.clone();
            let mut preparation=tokio::task::spawn_blocking(move||p.prepare(item));
            let prepared=loop {
                tokio::select! {
                    result=&mut preparation => break result.map_err(|_|"prepare_worker_failed".to_string())??,
                    _=tokio::time::sleep(Duration::from_millis(10)) => {
                        // If cancelled while decoding, await the worker before releasing
                        // the single-request guard, preventing overlapping preparations.
                        if let Err(error)=check(restored) { let _=preparation.await; return Err(error); }
                    }
                }
            };
            check(restored)?;
            if !restored {
                platform.may_begin(&target)?;
                *focus_started.lock().unwrap()=Some(Instant::now());
                event("restoringFocus",done,None);
                platform.may_begin(&target)?;
                check(false)?;
                ticket.cancel.dispatch(||platform.request_restore(&target))?;
                event("focusRequested",done,None);
                wait_until(&ticket.cancel,Duration::from_millis(600),"restore_foreground_timeout",||platform.restoration_status(&target)).await?;
                restored=true;
                event("focusReady",done,None);
                check(true)?;
                ticket.cancel.target_restored.store(true,Ordering::Release);
                // Keep the ordinary focusable/topmost panel visible. No hide
                // callback and no post-paste focus restoration to the panel.
                event("panelRetained",done,None);
            }
            check(true)?;
            event("writingClipboard",done,None);
            platform.write_clipboard(prepared)?;
            check(true)?;
            event("dispatchingPaste",done,None);
            check(true)?;
            ticket.cancel.dispatch(||platform.dispatch(&target))?;
            done+=1;
            event("waiting",done,None);
            // Include final settling interval: keep guard and Esc until target had
            // its configured read time. 'done' counts dispatched commands only.
            let until=Instant::now()+interval;
            while Instant::now()<until {
                check(true)?;
                tokio::time::sleep(Duration::from_millis(10).min(until.saturating_duration_since(Instant::now()))).await;
            }
            check(true)?;
        }
        Ok(())
    }.await;
    match result {
        Ok(()) => event("completed", done, None),
        Err(error) if error == "cancelled" => event("cancelled", done, None),
        Err(error) => event("failed", done, Some(error)),
    }
}

async fn wait_until(
    cancel: &Cancellation,
    timeout: Duration,
    timeout_error: &str,
    observe: impl Fn() -> Result<bool, String>,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        if Instant::now() >= deadline {
            return Err(timeout_error.into());
        }
        if observe()? {
            return Ok(());
        }
        tokio::time::sleep(
            Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    #[derive(Default)]
    struct Mock {
        sent: AtomicUsize,
        written: AtomicUsize,
        restores: AtomicUsize,
        wrong: AtomicBool,
        closed: AtomicBool,
        deny: AtomicBool,
        fail_write: AtomicBool,
        fail_prepare: AtomicBool,
        slow: AtomicBool,
        prepare_entered: Mutex<Option<std::sync::mpsc::Sender<()>>>,
        prepare_release: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
        restore_pending: AtomicUsize,
        restore_forever: AtomicBool,
        switch_during_restore: AtomicBool,
        panel: AtomicBool,
        panel_visible: AtomicBool,
        captures: AtomicUsize,
    }
    impl PastePlatform for Mock {
        type Target = ();
        type Prepared = ();
        fn capture_target(&self) -> Result<(), String> {
            self.captures.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn may_begin(&self, _: &()) -> Result<(), String> {
            if self.closed.load(Ordering::SeqCst) {
                Err("target_closed".into())
            } else if self.wrong.load(Ordering::SeqCst) {
                Err("foreground_changed_stop".into())
            } else {
                Ok(())
            }
        }
        fn target_is_foreground(&self, _: &()) -> Result<(), String> {
            if self.closed.load(Ordering::SeqCst) {
                Err("target_closed".into())
            } else if self.wrong.load(Ordering::SeqCst) {
                Err("foreground_changed_stop".into())
            } else if self.panel.load(Ordering::SeqCst) {
                Err("panel_interaction_stop".into())
            } else {
                Ok(())
            }
        }
        fn request_restore(&self, _: &()) -> Result<(), String> {
            self.restores.fetch_add(1, Ordering::SeqCst);
            if self.deny.load(Ordering::SeqCst) {
                Err("set_foreground_denied_no_paste".into())
            } else {
                self.may_begin(&())
            }
        }
        fn restoration_status(&self, _: &()) -> Result<bool, String> {
            self.may_begin(&())?;
            if self.switch_during_restore.swap(false, Ordering::SeqCst) {
                self.wrong.store(true, Ordering::SeqCst);
                return Ok(false);
            }
            if self.restore_forever.load(Ordering::SeqCst) {
                return Ok(false);
            }
            let ready = self
                .restore_pending
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_err();
            if ready {
                self.panel.store(false, Ordering::SeqCst);
            }
            Ok(ready)
        }
        fn prepare(&self, _: PasteItem) -> Result<(), String> {
            if let Some(entered) = self.prepare_entered.lock().unwrap().take() {
                entered.send(()).unwrap();
            }
            if let Some(release) = self.prepare_release.lock().unwrap().take() {
                release.recv().unwrap();
            }
            if self.slow.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(60));
            }
            if self.fail_prepare.load(Ordering::SeqCst) {
                Err("decode_failed".into())
            } else {
                Ok(())
            }
        }
        fn write_clipboard(&self, _: ()) -> Result<(), String> {
            self.written.fetch_add(1, Ordering::SeqCst);
            if self.fail_write.load(Ordering::SeqCst) {
                Err("clipboard_busy".into())
            } else {
                Ok(())
            }
        }
        fn dispatch(&self, _: &()) -> Result<(), String> {
            self.target_is_foreground(&())?;
            self.sent.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    fn items(n: usize) -> Vec<PasteItem> {
        (0..n).map(|i| PasteItem::Text(i.to_string())).collect()
    }
    #[tokio::test]
    async fn visible_panel_continuous_calls_keep_target_and_do_not_refocus_panel() {
        let p = Arc::new(Mock::default());
        p.panel_visible.store(true, Ordering::SeqCst);
        let q = Queue::default();
        for _ in 0..2 {
            // The next explicit click activates the editable panel; no new capture.
            p.panel.store(true, Ordering::SeqCst);
            p.restore_pending.store(2, Ordering::SeqCst);
            let phases = Mutex::new(Vec::new());
            run(
                p.clone(),
                (),
                q.reserve().unwrap(),
                "c".into(),
                items(1),
                Duration::ZERO,
                |e| phases.lock().unwrap().push(e.phase),
            )
            .await;
            assert!(p.panel_visible.load(Ordering::SeqCst));
            assert!(!p.panel.load(Ordering::SeqCst));
            assert!(phases.lock().unwrap().iter().any(|p| p == "panelRetained"));
            assert!(!phases
                .lock()
                .unwrap()
                .iter()
                .any(|p| p.contains("hide") || p.contains("Hidden")));
            q.release();
        }
        assert_eq!(p.restores.load(Ordering::SeqCst), 2);
        assert_eq!(p.sent.load(Ordering::SeqCst), 2);
        assert_eq!(p.captures.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn panel_refocus_after_send_stops_queue_without_restoring_again() {
        let p = Arc::new(Mock::default());
        p.panel_visible.store(true, Ordering::SeqCst);
        let q = Queue::default();
        let last = Mutex::new(None);
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(5),
            Duration::from_millis(20),
            |e| {
                if e.done == 1 {
                    p.panel.store(true, Ordering::SeqCst);
                }
                *last.lock().unwrap() = Some(e);
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 1);
        assert_eq!(p.restores.load(Ordering::SeqCst), 1);
        assert!(p.panel_visible.load(Ordering::SeqCst));
        assert_eq!(
            last.lock().unwrap().as_ref().unwrap().error.as_deref(),
            Some("panel_interaction_stop")
        );
    }
    #[tokio::test]
    async fn third_party_during_first_preparation_never_activates_or_sends() {
        let p = Arc::new(Mock::default());
        p.slow.store(true, Ordering::SeqCst);
        let q = Queue::default();
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(1),
            Duration::ZERO,
            |e| {
                if e.phase == "preparing" {
                    p.wrong.store(true, Ordering::SeqCst);
                }
            },
        )
        .await;
        assert_eq!(p.restores.load(Ordering::SeqCst), 0);
        assert_eq!(p.written.load(Ordering::SeqCst), 0);
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn restored_native_refocus_cancels_but_initial_activation_notification_does_not() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        let ticket = q.reserve().unwrap();
        let id = ticket.id;
        assert!(!q.cancel_panel_refocus());
        run(
            p.clone(),
            (),
            ticket,
            "c".into(),
            items(1),
            Duration::ZERO,
            |e| {
                if e.phase == "panelRetained" {
                    assert!(q.cancel_panel_refocus());
                }
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
        q.release();
        let next = q.reserve().unwrap();
        assert_ne!(id, next.id);
        assert!(!q.cancel_request(id));
        assert!(!next.cancel.is_cancelled());
    }
    #[tokio::test]
    async fn activation_timeout_never_hides_writes_or_retries() {
        let p = Arc::new(Mock::default());
        p.restore_forever.store(true, Ordering::SeqCst);
        let q = Queue::default();
        let last = Mutex::new(None);
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(1),
            Duration::ZERO,
            |e| *last.lock().unwrap() = Some(e),
        )
        .await;
        assert_eq!(
            last.lock().unwrap().as_ref().unwrap().error.as_deref(),
            Some("restore_foreground_timeout")
        );
        assert_eq!(p.restores.load(Ordering::SeqCst), 1);
        assert_eq!(p.written.load(Ordering::SeqCst), 0);
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn third_party_during_activation_wait_rejects_without_retry() {
        let p = Arc::new(Mock::default());
        p.switch_during_restore.store(true, Ordering::SeqCst);
        let q = Queue::default();
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(1),
            Duration::ZERO,
            |_| {},
        )
        .await;
        assert_eq!(p.restores.load(Ordering::SeqCst), 1);
        assert_eq!(p.written.load(Ordering::SeqCst), 0);
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn cancellation_during_activation_wait_is_prompt_and_never_sends() {
        let p = Arc::new(Mock::default());
        p.restore_forever.store(true, Ordering::SeqCst);
        let q = Queue::default();
        let ticket = q.reserve().unwrap();
        let cancellation = ticket.cancel.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let started_tx = Mutex::new(Some(started_tx));
        let canceller = std::thread::spawn(move || {
            started_rx.recv().unwrap();
            std::thread::sleep(Duration::from_millis(25));
            cancellation.cancel();
        });
        let start = Instant::now();
        let last = Mutex::new(None);
        run(
            p.clone(),
            (),
            ticket,
            "c".into(),
            items(1),
            Duration::ZERO,
            |e| {
                if e.phase == "focusRequested" {
                    started_tx.lock().unwrap().take().unwrap().send(()).unwrap();
                }
                *last.lock().unwrap() = Some(e);
            },
        )
        .await;
        canceller.join().unwrap();
        assert!(start.elapsed() < Duration::from_millis(400));
        assert_eq!(last.lock().unwrap().as_ref().unwrap().phase, "cancelled");
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn six_in_order_one_restore_and_truthful_done() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        let events = Mutex::new(Vec::new());
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "case".into(),
            items(6),
            Duration::ZERO,
            |e| events.lock().unwrap().push(e),
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 6);
        assert_eq!(p.restores.load(Ordering::SeqCst), 1);
        let events = events.lock().unwrap();
        assert_eq!(events.last().unwrap().phase, "completed");
        assert_eq!(events.last().unwrap().done, 6);
        assert_eq!(
            events
                .iter()
                .filter(|e| e.phase == "waiting")
                .map(|e| e.done)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
    }
    #[tokio::test]
    async fn cancel_second_send_stops_remainder() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        let last = Mutex::new(None);
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(6),
            Duration::from_millis(30),
            |e| {
                if e.done == 2 {
                    q.cancel();
                }
                *last.lock().unwrap() = Some(e);
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 2);
        assert_eq!(last.lock().unwrap().as_ref().unwrap().phase, "cancelled");
    }
    #[tokio::test]
    async fn focus_switched_does_not_restore_again() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(4),
            Duration::from_millis(20),
            |e| {
                if e.done == 1 {
                    p.wrong.store(true, Ordering::SeqCst);
                }
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 1);
        assert_eq!(p.restores.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn refusal_closed_clipboard_and_prepare_never_send() {
        for mode in 0..4 {
            let p = Arc::new(Mock::default());
            match mode {
                0 => p.deny.store(true, Ordering::SeqCst),
                1 => p.closed.store(true, Ordering::SeqCst),
                2 => p.fail_write.store(true, Ordering::SeqCst),
                _ => p.fail_prepare.store(true, Ordering::SeqCst),
            };
            let q = Queue::default();
            run(
                p.clone(),
                (),
                q.reserve().unwrap(),
                "c".into(),
                items(4),
                Duration::ZERO,
                |_| {},
            )
            .await;
            assert_eq!(p.sent.load(Ordering::SeqCst), 0);
        }
    }
    #[tokio::test]
    async fn cancel_on_write_boundary_no_dispatch() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(1),
            Duration::ZERO,
            |e| {
                if e.phase == "dispatchingPaste" {
                    q.cancel();
                }
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
        assert_eq!(p.written.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn cancel_during_prepare_waits_worker_and_no_send() {
        let p = Arc::new(Mock::default());
        let q = Arc::new(Queue::default());
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        *p.prepare_entered.lock().unwrap() = Some(entered_tx);
        *p.prepare_release.lock().unwrap() = Some(release_rx);
        let ticket = q.reserve().unwrap();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_test = completed.clone();
        let queue_test = q.clone();
        let canceller = std::thread::spawn(move || {
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(queue_test.cancel());
            std::thread::sleep(Duration::from_millis(30));
            assert!(!completed_test.load(Ordering::SeqCst));
            assert!(queue_test.reserve().is_err());
            release_tx.send(()).unwrap();
        });
        run(
            p.clone(),
            (),
            ticket,
            "c".into(),
            items(1),
            Duration::ZERO,
            |_| {},
        )
        .await;
        completed.store(true, Ordering::SeqCst);
        canceller.join().unwrap();
        q.release();
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
        assert!(q.reserve().is_ok());
    }
    #[tokio::test]
    async fn focus_switched_at_restore_event_never_restores_or_sends() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(4),
            Duration::ZERO,
            |e| {
                if e.phase == "restoringFocus" {
                    p.wrong.store(true, Ordering::SeqCst);
                }
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 0);
        assert_eq!(p.restores.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn closed_during_wait_stops_remainder() {
        let p = Arc::new(Mock::default());
        let q = Queue::default();
        let last = Mutex::new(None);
        run(
            p.clone(),
            (),
            q.reserve().unwrap(),
            "c".into(),
            items(4),
            Duration::from_millis(20),
            |e| {
                if e.done == 1 {
                    p.closed.store(true, Ordering::SeqCst);
                }
                *last.lock().unwrap() = Some(e);
            },
        )
        .await;
        assert_eq!(p.sent.load(Ordering::SeqCst), 1);
        assert_eq!(
            last.lock().unwrap().as_ref().unwrap().error.as_deref(),
            Some("target_closed")
        );
    }
    #[test]
    fn single_guard_all_request_kinds_until_release() {
        let q = Queue::default();
        assert!(q.reserve().is_ok());
        assert!(q.reserve().is_err());
        assert!(q.cancel());
        assert!(q.reserve().is_err());
        q.release();
        assert!(q.reserve().is_ok());
    }
    #[test]
    fn concurrent_cancel_is_linearized_with_dispatch() {
        // A real second thread cancels while a dispatch owns the gate. Cancel
        // acknowledges only after this already-started send completes.
        let q = Arc::new(Queue::default());
        let ticket = q.reserve().unwrap();
        let entered = Arc::new(std::sync::Barrier::new(2));
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let cancellation = ticket.cancel.clone();
        let inside = entered.clone();
        let sender = std::thread::spawn(move || {
            cancellation.dispatch(|| {
                inside.wait();
                finish_rx.recv().unwrap();
                Ok(())
            })
        });
        entered.wait();
        let cancel_q = q.clone();
        let accepted = Arc::new(AtomicBool::new(false));
        let done = accepted.clone();
        let canceller = std::thread::spawn(move || {
            cancel_q.cancel();
            done.store(true, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(10));
        assert!(!accepted.load(Ordering::SeqCst));
        finish_tx.send(()).unwrap();
        sender.join().unwrap().unwrap();
        canceller.join().unwrap();
        assert!(accepted.load(Ordering::SeqCst));
        let called = AtomicBool::new(false);
        assert_eq!(
            ticket.cancel.dispatch(|| {
                called.store(true, Ordering::SeqCst);
                Ok(())
            }),
            Err("cancelled".into())
        );
        assert!(!called.load(Ordering::SeqCst));
    }
}
