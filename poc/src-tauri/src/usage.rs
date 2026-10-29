//! Per-request recording gate. Storage failure is reported once; it never replays dispatch.
#[derive(Default)]
pub struct FirstDispatch(std::sync::atomic::AtomicBool);
impl crate::store::Store{
    pub fn record_use(&self,case_id:&str)->Result<(),String>{
        let timestamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_millis() as i64;
        let connection=self.connection.lock().map_err(|e|e.to_string())?;
        let changed=connection.execute("UPDATE cases SET use_count=use_count+1,last_used_at=?1 WHERE id=?2 AND deleted_at IS NULL",rusqlite::params![timestamp,case_id]).map_err(|e|format!("sqlite: {e}"))?;
        if changed!=1{return Err("case_missing_or_deleted".into())}Ok(())
    }
}
impl FirstDispatch{
    pub fn observe(&self,done:usize,record:impl FnOnce()->Result<(),String>)->Option<Result<(),String>>{
        if done==0||self.0.swap(true,std::sync::atomic::Ordering::AcqRel){None}else{Some(record())}
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn zero_dispatch_does_not_record_and_every_later_phase_records_only_once(){let gate=FirstDispatch::default();let count=std::cell::Cell::new(0);for done in [0,0,1,2,2,2]{gate.observe(done,||{count.set(count.get()+1);Ok(())});}assert_eq!(count.get(),1);}
    #[test]fn partially_cancelled_request_still_counts_and_error_does_not_retry_or_replay(){let gate=FirstDispatch::default();let calls=std::cell::Cell::new(0);assert!(gate.observe(1,||{calls.set(calls.get()+1);Err("disk_error".into())}).unwrap().is_err());assert!(gate.observe(1,||{calls.set(calls.get()+1);Ok(())}).is_none());assert_eq!(calls.get(),1);}
}
