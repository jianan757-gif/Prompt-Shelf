use serde::Serialize;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseRequest {
    pub token: u64,
    pub action: String,
}
#[derive(Default)]
pub struct CloseProtocol {
    next: u64,
    pub pending: Option<CloseRequest>,
    finishing: bool,
}
impl CloseProtocol {
    pub fn editing_epoch(&self, edits: &crate::cases::EditSessions) -> Result<u64, String> {
        if self.pending.is_some() {
            return Err("library_close_pending".into());
        }
        Ok(edits.epoch())
    }
    pub fn request(&mut self, action: &str) -> CloseRequest {
        if let Some(request) = &mut self.pending {
            if action == "exit" && !self.finishing {
                request.action = "exit".into()
            }
            return request.clone();
        }
        self.next += 1;
        let request = CloseRequest {
            token: self.next,
            action: action.into(),
        };
        self.pending = Some(request.clone());
        request
    }
    pub fn get(&self, token: u64) -> Result<CloseRequest, String> {
        self.pending
            .as_ref()
            .filter(|r| r.token == token)
            .cloned()
            .ok_or("close_request_expired".into())
    }
    pub fn cancel(&mut self, token: u64) -> Result<(), String> {
        self.get(token)?;
        if self.finishing {
            return Err("library_close_finishing".into());
        }
        self.pending = None;
        Ok(())
    }
    pub fn claim(&mut self, token: u64) -> Result<CloseRequest, String> {
        let request = self.get(token)?;
        if self.finishing {
            return Err("library_close_finishing".into());
        }
        self.finishing = true;
        Ok(request)
    }
    pub fn failed(&mut self, token: u64) {
        if self.pending.as_ref().is_some_and(|r| r.token == token) {
            self.reset();
        }
    }
    pub fn delivery_failed(&mut self, token: u64) {
        if !self.finishing {
            self.failed(token)
        }
    }
    pub fn reset(&mut self) {
        self.pending = None;
        self.finishing = false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_requests_escalate_exit_and_failed_ack_keeps_request() {
        let mut p = CloseProtocol::default();
        let a = p.request("close");
        assert_eq!(p.request("exit").token, a.token);
        assert_eq!(p.get(a.token).unwrap().action, "exit");
        assert!(p.get(a.token + 1).is_err());
        assert!(p.pending.is_some());
        p.cancel(a.token).unwrap();
        assert!(p.get(a.token).is_err());
        assert_ne!(p.request("close").token, a.token);
    }
    #[test]
    fn pending_close_blocks_new_epoch_until_user_continues() {
        let mut p = CloseProtocol::default();
        let edits = crate::cases::EditSessions::default();
        let before = p.editing_epoch(&edits).unwrap();
        let r = p.request("close");
        edits.prepare_close().unwrap();
        assert!(edits.epoch() > before);
        assert!(p.editing_epoch(&edits).is_err());
        p.cancel(r.token).unwrap();
        assert!(p.editing_epoch(&edits).is_ok());
    }
    #[test]
    fn finishing_rejects_cancel_and_failure_clears_request_for_retry() {
        let mut p = CloseProtocol::default();
        let edits = crate::cases::EditSessions::default();
        let r = p.request("close");
        p.claim(r.token).unwrap();
        edits.prepare_close().unwrap();
        assert!(p.cancel(r.token).is_err());
        assert!(p.editing_epoch(&edits).is_err());
        assert!(p.claim(r.token).is_err());
        p.failed(r.token);
        assert!(p.claim(r.token).is_err());
        assert!(p.editing_epoch(&edits).is_ok());
        let again = p.request("exit");
        p.claim(again.token).unwrap();
        p.reset();
        assert!(p.pending.is_none());
        assert!(p.editing_epoch(&edits).is_ok());
    }
    #[test]
    fn delivery_failure_unlocks_editing_but_cannot_undo_claimed_close() {
        let mut p = CloseProtocol::default();
        let edits = crate::cases::EditSessions::default();
        let a = p.request("close");
        p.delivery_failed(a.token);
        assert!(p.editing_epoch(&edits).is_ok());
        assert!(p.claim(a.token).is_err());
        let b = p.request("close");
        assert_ne!(a.token, b.token);
        p.claim(b.token).unwrap();
        p.delivery_failed(b.token);
        assert!(p.editing_epoch(&edits).is_err());
        assert_eq!(p.request("exit").action, "close");
        assert!(p.cancel(b.token).is_err());
    }
}
