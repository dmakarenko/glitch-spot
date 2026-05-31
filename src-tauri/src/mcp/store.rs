// ABOUTME: In-memory snapshot store for the MCP server.
// ABOUTME: Broadcasts snapshot-ready and cancel events to waiting tool callers.

use crate::snapshot::Snapshot;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tokio::time::{timeout, Duration};

#[derive(Clone, Debug, PartialEq)]
pub enum StoreEvent {
    Snapshot(Snapshot),
    Cancelled,
}

pub struct SnapshotStore {
    latest: Arc<Mutex<Option<Snapshot>>>,
    // id -> snapshot, for get_snapshot(id) lookups
    all: Arc<Mutex<HashMap<String, Snapshot>>>,
    tx: watch::Sender<Option<StoreEvent>>,
    rx: watch::Receiver<Option<StoreEvent>>, // kept to prevent sender-dropped errors on clone
}

pub enum WaitResult {
    Snapshot(Snapshot),
    Cancelled,
    TimedOut,
}

impl SnapshotStore {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(None);
        Self {
            latest: Arc::new(Mutex::new(None)),
            all: Arc::new(Mutex::new(HashMap::new())),
            tx,
            rx,
        }
    }

    // NOTE: watch::channel retains only the most recent value. If publish() and cancel()
    // are called back-to-back before a waiter polls, the waiter sees Cancelled, not the
    // snapshot. This is acceptable for our use case (cancel is user-initiated and only
    // called when no annotation is expected).
    pub fn publish(&self, snap: Snapshot) {
        {
            let mut latest = self.latest.lock().unwrap();
            *latest = Some(snap.clone());
        }
        self.all.lock().unwrap().insert(snap.id.clone(), snap.clone());
        // Ignore send error — it means no receivers, which is fine.
        let _ = self.tx.send(Some(StoreEvent::Snapshot(snap)));
    }

    // NOTE: Sends a Cancelled event; does not check if any waiter is active. No-op if
    // no one is waiting (the sent value will just be the current watch channel state).
    pub fn cancel(&self) {
        let _ = self.tx.send(Some(StoreEvent::Cancelled));
    }

    pub fn get_latest(&self) -> Option<Snapshot> {
        self.latest.lock().unwrap().clone()
    }

    pub fn get_snapshot(&self, id: &str) -> Option<Snapshot> {
        self.all.lock().unwrap().get(id).cloned()
    }

    /// Removes snapshots from the store whose `{base}/{id}/` directory no longer
    /// exists on disk. Clears `latest` if it was among the removed entries.
    /// Returns the number of entries removed.
    pub fn expire_if_deleted(&self, base: &Path) -> usize {
        let mut all = self.all.lock().unwrap();
        let before = all.len();
        all.retain(|id, _| base.join(id).is_dir());
        let removed = before - all.len();
        if removed > 0 {
            let mut latest = self.latest.lock().unwrap();
            if let Some(snap) = latest.as_ref() {
                if !base.join(&snap.id).is_dir() {
                    *latest = None;
                }
            }
        }
        removed
    }

    /// Wait for the next snapshot or cancel signal, up to `timeout_secs`.
    pub async fn wait_for_next(&self, timeout_secs: u64) -> WaitResult {
        let mut rx = self.rx.clone();
        // Mark current value as seen so changed() only fires on the NEXT send.
        let _ = rx.borrow_and_update();

        let wait = async {
            loop {
                if rx.changed().await.is_err() {
                    // Sender dropped — treat as timeout.
                    return WaitResult::TimedOut;
                }
                match rx.borrow().as_ref() {
                    Some(StoreEvent::Snapshot(s)) => return WaitResult::Snapshot(s.clone()),
                    Some(StoreEvent::Cancelled) => return WaitResult::Cancelled,
                    None => continue, // defensive: should not happen with current senders
                }
            }
        };

        match timeout(Duration::from_secs(timeout_secs), wait).await {
            Ok(result) => result,
            Err(_) => WaitResult::TimedOut,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};

    fn fake_snapshot(id: &str) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            mode: "frozen".to_string(),
            annotations: vec![Annotation {
                id: "a1".to_string(),
                shape: "rect".to_string(),
                coords: Coords { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
                note: "fix this".to_string(),
                label: "01".to_string(),
            }],
            context: Context {
                screen: ScreenInfo { width: 1440, height: 900, scale_factor: 2.0, display_index: 0 },
                os: OsInfo { platform: "macos".to_string(), version: "15.4.0".to_string() },
                window: None,
            },
            images: Images { clean: "/tmp/clean.png".to_string(), annotated: None },
        }
    }

    #[tokio::test]
    async fn get_latest_returns_none_initially() {
        let store = SnapshotStore::new();
        assert!(store.get_latest().is_none());
    }

    #[tokio::test]
    async fn publish_updates_latest() {
        let store = SnapshotStore::new();
        let snap = fake_snapshot("snap_001");
        store.publish(snap.clone());
        assert_eq!(store.get_latest().unwrap().id, "snap_001");
    }

    #[tokio::test]
    async fn wait_receives_published_snapshot() {
        let store = Arc::new(SnapshotStore::new());
        let store2 = store.clone();

        let handle = tokio::spawn(async move {
            store2.wait_for_next(5).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        store.publish(fake_snapshot("snap_002"));

        match handle.await.unwrap() {
            WaitResult::Snapshot(s) => assert_eq!(s.id, "snap_002"),
            other => panic!("expected Snapshot, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn wait_returns_cancelled_on_cancel() {
        let store = Arc::new(SnapshotStore::new());
        let store2 = store.clone();

        let handle = tokio::spawn(async move {
            store2.wait_for_next(5).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        store.cancel();

        match handle.await.unwrap() {
            WaitResult::Cancelled => {}
            other => panic!("expected Cancelled, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn wait_times_out() {
        let store = SnapshotStore::new();
        match store.wait_for_next(0).await {
            WaitResult::TimedOut => {}
            other => panic!("expected TimedOut, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn get_snapshot_by_id() {
        let store = SnapshotStore::new();
        store.publish(fake_snapshot("snap_abc"));
        assert!(store.get_snapshot("snap_abc").is_some());
        assert!(store.get_snapshot("snap_xyz").is_none());
    }

    #[test]
    fn expire_if_deleted_keeps_snapshot_with_existing_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let id = "snap_keep";
        std::fs::create_dir(temp.path().join(id)).unwrap();
        let store = SnapshotStore::new();
        store.publish(fake_snapshot(id));
        let expired = store.expire_if_deleted(temp.path());
        assert_eq!(expired, 0, "dir still exists — nothing should be expired");
        assert!(store.get_latest().is_some());
        assert!(store.get_snapshot(id).is_some());
    }

    #[test]
    fn expire_if_deleted_clears_latest_when_dir_is_gone() {
        let temp = tempfile::TempDir::new().unwrap();
        let id = "snap_gone";
        // Publish but do NOT create the directory — simulates post-cleanup state.
        let store = SnapshotStore::new();
        store.publish(fake_snapshot(id));
        let expired = store.expire_if_deleted(temp.path());
        assert_eq!(expired, 1);
        assert!(store.get_latest().is_none(), "latest should be cleared");
        assert!(store.get_snapshot(id).is_none(), "all-map entry should be removed");
    }

    #[test]
    fn expire_if_deleted_with_no_snapshots_is_noop() {
        let temp = tempfile::TempDir::new().unwrap();
        let store = SnapshotStore::new();
        let expired = store.expire_if_deleted(temp.path());
        assert_eq!(expired, 0);
        assert!(store.get_latest().is_none());
    }

    #[tokio::test]
    async fn publishing_multiple_snapshots_each_wakes_a_waiter() {
        let store = Arc::new(SnapshotStore::new());

        for i in 0..3u32 {
            let store2 = store.clone();
            let handle = tokio::spawn(async move { store2.wait_for_next(5).await });
            tokio::time::sleep(Duration::from_millis(20)).await;
            store.publish(fake_snapshot(&format!("snap_{i:03}")));
            match handle.await.unwrap() {
                WaitResult::Snapshot(s) => assert_eq!(s.id, format!("snap_{i:03}")),
                _ => panic!("expected snapshot"),
            }
        }
    }
}
