use super::*;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

#[derive(Default)]
struct Backend {
    object: Option<Object>,
    fault: u8,
    delayed: Option<(Option<String>, Vec<u8>)>,
}
#[derive(Clone, Default)]
struct Store(Rc<RefCell<Backend>>);
impl Store {
    fn apply(
        backend: &mut Backend,
        expected: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        if backend.object.as_ref().map(|o| o.etag.as_str()) != expected {
            return Err(StoreError::Conflict);
        }
        backend.object = Some(Object {
            bytes: bytes.to_vec(),
            etag: signed::digest(bytes),
        });
        Ok(())
    }
    fn flush(&self) -> std::result::Result<(), StoreError> {
        let (etag, bytes) = self.0.borrow_mut().delayed.take().unwrap();
        Self::apply(&mut self.0.borrow_mut(), etag.as_deref(), &bytes)
    }
}
impl ObjectStore for Store {
    fn lock_identity(&self) -> Option<String> {
        Some(format!("r2-lock:{}", "a".repeat(64)))
    }
    fn channel(&self) -> switcher_domain::ads::AdChannel {
        switcher_domain::ads::AdChannel::Preview
    }
    fn read_lock(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        Ok(self.0.borrow().object.clone())
    }
    fn commit_lock(
        &mut self,
        expected: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError> {
        let mut backend = self.0.borrow_mut();
        match std::mem::take(&mut backend.fault) {
            1 => {
                backend.delayed = Some((expected.map(str::to_owned), bytes.to_vec()));
                Err(StoreError::OutcomeUnknown)
            }
            2 => {
                Self::apply(&mut backend, expected, bytes)?;
                Err(StoreError::OutcomeUnknown)
            }
            _ => Self::apply(&mut backend, expected, bytes),
        }
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError> {
        panic!("lock protocol must not access business objects")
    }
    fn public_read(
        &mut self,
        _: &str,
        _: usize,
    ) -> std::result::Result<Option<Object>, StoreError> {
        unreachable!()
    }
    fn put_immutable(&mut self, _: &str, _: &[u8]) -> std::result::Result<(), StoreError> {
        unreachable!()
    }
    fn commit_current(&mut self, _: Option<&str>, _: &[u8]) -> std::result::Result<(), StoreError> {
        unreachable!()
    }
}
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn publisher() -> (Root, Publisher) {
    let path = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("switcher-lock-{}", random_id().unwrap()));
    let publisher = Publisher::open(&path, "test", None, 1000).unwrap();
    (Root(path), publisher)
}

#[test]
fn different_owners_cannot_take_over_and_release_uses_unique_tombstones() {
    let (_a, a) = publisher();
    let (_b, b) = publisher();
    let mut store = Store::default();
    let lease = Lease::acquire(&a, &mut store).unwrap();
    assert!(matches!(
        Lease::acquire(&b, &mut store),
        Err(Error::Store(StoreError::Conflict))
    ));
    assert!(b.lock_journal().unwrap().owners.is_empty());
    lease.release(&a, &mut store).unwrap();
    let first = store.0.borrow().object.as_ref().unwrap().etag.clone();
    Lease::acquire(&b, &mut store)
        .unwrap()
        .release(&b, &mut store)
        .unwrap();
    assert_ne!(first, store.0.borrow().object.as_ref().unwrap().etag);
    a.require_no_remote_intents().unwrap();
    b.require_no_remote_intents().unwrap();
}

#[test]
fn restart_recovers_own_lock_and_fences_delayed_acquisition() {
    let (root, a) = publisher();
    let mut store = Store::default();
    store.0.borrow_mut().fault = 1;
    assert!(Lease::acquire(&a, &mut store).is_err());
    assert!(a.require_no_remote_intents().is_err());
    drop(a);
    let a = Publisher::open(&root.0, "test", None, 1000).unwrap();
    let lease = Lease::acquire(&a, &mut store).unwrap();
    assert_eq!(store.flush(), Err(StoreError::Conflict));
    lease.verify(&mut store).unwrap();
    lease.release(&a, &mut store).unwrap();
    a.require_no_remote_intents().unwrap();
}

#[test]
fn delayed_acquisition_that_wins_is_recovered_before_business_can_start() {
    let (_root, p) = publisher();
    let mut store = Store::default();
    store.0.borrow_mut().fault = 1;
    assert!(Lease::acquire(&p, &mut store).is_err());
    store.flush().unwrap();
    let lease = Lease::acquire(&p, &mut store).unwrap();
    lease.verify(&mut store).unwrap();
    lease.release(&p, &mut store).unwrap();
    p.require_no_remote_intents().unwrap();
}

#[test]
fn unknown_committed_acquisition_and_release_remain_recoverable() {
    let (_root, p) = publisher();
    let mut store = Store::default();
    store.0.borrow_mut().fault = 2;
    assert!(Lease::acquire(&p, &mut store).is_err());
    let lease = Lease::acquire(&p, &mut store).unwrap();
    store.0.borrow_mut().fault = 2;
    assert!(lease.release(&p, &mut store).is_err());
    assert!(p.require_no_remote_intents().is_err());
    Lease::acquire(&p, &mut store)
        .unwrap()
        .release(&p, &mut store)
        .unwrap();
    p.require_no_remote_intents().unwrap();
}

#[test]
fn delayed_release_and_stale_handles_cannot_unlock_new_operation() {
    let (_root, p) = publisher();
    let mut store = Store::default();
    let stale = Lease::acquire(&p, &mut store).unwrap();
    store.0.borrow_mut().fault = 1;
    assert!(stale.release(&p, &mut store).is_err());
    let old = Lease::acquire(&p, &mut store).unwrap();
    let current = Lease::acquire(&p, &mut store).unwrap();
    assert!(old.verify(&mut store).is_err());
    assert_eq!(store.flush(), Err(StoreError::Conflict));
    assert!(old.release(&p, &mut store).is_err());
    current.release(&p, &mut store).unwrap();
}

#[test]
fn malformed_record_or_journal_blocks_remote_mutations() {
    let (root, p) = publisher();
    let mut store = Store::default();
    for bytes in [
        b"{}".to_vec(),
        vec![b'x'; WIRE_LIMIT + 1],
        br#"{"schema":1,"owner":null,"revision":"bad"}"#.to_vec(),
        format!(r#"{{"schema":1,"revision":"{}"}}"#, "a".repeat(32)).into_bytes(),
        format!(
            r#"{{"schema":1,"owner":"","revision":"{}"}}"#,
            "a".repeat(32)
        )
        .into_bytes(),
    ] {
        store.0.borrow_mut().object = Some(Object {
            bytes: bytes.clone(),
            etag: "test".into(),
        });
        assert!(Lease::acquire(&p, &mut store).is_err());
        assert_eq!(store.0.borrow().object.as_ref().unwrap().bytes, bytes);
        assert!(p.lock_journal().unwrap().owners.is_empty());
    }
    store.0.borrow_mut().object = None;
    storage::atomic_write(&root.0.join(JOURNAL), b"{}").unwrap();
    assert!(Lease::acquire(&p, &mut store).is_err());
    assert!(store.0.borrow().object.is_none());
}

#[test]
fn invalid_etags_and_unknown_lock_fields_never_authorize_a_lease() {
    let (_root, p) = publisher();
    let mut store = Store::default();
    let bytes = encode(
        &Record {
            schema: 1,
            owner: None,
            revision: random_id().unwrap(),
        },
        WIRE_LIMIT,
    )
    .unwrap();
    for etag in [String::new(), "x".repeat(1025), "bad\netag".to_owned()] {
        store.0.borrow_mut().object = Some(Object {
            bytes: bytes.clone(),
            etag,
        });
        assert!(Lease::acquire(&p, &mut store).is_err());
        p.require_no_remote_intents().unwrap();
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["expires_at"] = 1.into();
    store.0.borrow_mut().object = Some(Object {
        bytes: serde_json::to_vec(&value).unwrap(),
        etag: "test".into(),
    });
    assert!(Lease::acquire(&p, &mut store).is_err());
    p.require_no_remote_intents().unwrap();
}

#[test]
fn journal_must_be_durable_before_remote_write_and_lost_ownership_fails_closed() {
    let (root, p) = publisher();
    let mut store = Store::default();
    std::fs::create_dir(root.0.join(JOURNAL)).unwrap();
    assert!(Lease::acquire(&p, &mut store).is_err());
    assert!(store.0.borrow().object.is_none());
    std::fs::remove_dir(root.0.join(JOURNAL)).unwrap();
    let lease = Lease::acquire(&p, &mut store).unwrap();
    store.0.borrow_mut().object = None;
    assert!(lease.verify(&mut store).is_err());
    assert!(lease.release(&p, &mut store).is_err());
    assert!(p.require_no_remote_intents().is_err());
}
