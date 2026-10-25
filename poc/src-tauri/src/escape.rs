/// Global Escape belongs to a visible quick-panel session, never a paste job.
#[derive(Default)]
pub struct EscapeSession {
    pub visible: bool,
    pub registered: bool,
    pub error: Option<String>,
    held: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source { Global, Local }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action { Ignore, Cancel, Hide }
impl EscapeSession {
    pub fn shown(&mut self, registration: Result<(), String>) {
        let continuing = self.visible && self.registered;
        self.visible = true;
        match registration {
            Ok(()) => { self.registered = true; self.error = None; }
            Err(error) => { self.registered = false; self.error = Some(error); }
        }
        if !continuing { self.held = false; }
    }
    pub fn unregistered(&mut self) {
        self.registered = false; self.held = false; self.error = None;
    }
    pub fn hidden(&mut self) -> Result<(), String> {
        if self.registered { return Err("cannot_hide_with_global_escape_owned".into()); }
        self.visible = false; self.held = false; Ok(())
    }
    pub fn can_paste(&self) -> bool { self.visible && self.registered }
    pub fn signal(&mut self, source: Source, pressed: bool, repeat: bool, busy: bool) -> Action {
        // Local duplicates never act while the global registration is authority.
        if !self.visible || (source == Source::Global) != self.registered { return Action::Ignore; }
        if !pressed { self.held = false; return Action::Ignore; }
        if repeat || self.held { return Action::Ignore; }
        self.held = true;
        if busy { Action::Cancel } else { Action::Hide }
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn job_completion_does_not_release_visible_session_escape() {
        let mut session=EscapeSession::default();session.shown(Ok(()));
        assert!(session.can_paste());
        assert_eq!(session.signal(Source::Global,true,false,false),Action::Hide);
        // Registration ends with explicit hide, not with job completion.
        session.unregistered();session.hidden().unwrap();assert!(!session.registered);
        assert_eq!(session.signal(Source::Global,true,false,false),Action::Ignore);
    }
    #[test] fn one_held_escape_cancels_without_later_hiding_idle_panel() {
        let mut session=EscapeSession::default();session.shown(Ok(()));
        assert_eq!(session.signal(Source::Global,true,false,true),Action::Cancel);
        assert_eq!(session.signal(Source::Global,true,true,false),Action::Ignore);
        assert_eq!(session.signal(Source::Global,true,false,false),Action::Ignore);
        assert_eq!(session.signal(Source::Local,true,false,false),Action::Ignore);
        assert_eq!(session.signal(Source::Local,false,false,false),Action::Ignore);
        assert_eq!(session.signal(Source::Global,false,false,false),Action::Ignore);
        assert!(session.visible && session.registered);
        assert_eq!(session.signal(Source::Global,true,false,false),Action::Hide);
    }
    #[test] fn registration_failure_is_explicit_and_local_fallback_is_one_press() {
        let mut session=EscapeSession::default();session.shown(Err("conflict".into()));
        assert!(!session.can_paste());assert_eq!(session.error.as_deref(),Some("conflict"));
        assert_eq!(session.signal(Source::Global,true,false,false),Action::Ignore);
        assert_eq!(session.signal(Source::Local,true,false,true),Action::Cancel);
        assert_eq!(session.signal(Source::Local,true,true,false),Action::Ignore);
        session.signal(Source::Local,false,false,false);
        assert_eq!(session.signal(Source::Local,true,false,false),Action::Hide);
    }
    #[test] fn unregister_failure_cannot_mark_panel_hidden_or_release_ownership() {
        let mut session=EscapeSession::default();session.shown(Ok(()));
        assert!(session.hidden().is_err());assert!(session.visible && session.registered);
        session.unregistered();session.hidden().unwrap();assert!(!session.visible);
        session.shown(Ok(()));assert!(session.can_paste());
    }
    #[test] fn repeated_wake_does_not_reset_a_held_cancel_press() {
        let mut session=EscapeSession::default();session.shown(Ok(()));
        assert_eq!(session.signal(Source::Global,true,false,true),Action::Cancel);
        session.shown(Ok(()));
        assert_eq!(session.signal(Source::Global,true,false,false),Action::Ignore);
    }
}
