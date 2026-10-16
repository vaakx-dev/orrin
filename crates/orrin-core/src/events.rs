use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::broadcast;

use crate::api::{Event, EventEnvelope};
use crate::clock::now_ms;

#[derive(Clone)]
pub struct Events {
    tx: broadcast::Sender<EventEnvelope>,
    seq: Arc<AtomicU64>,
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

impl Events {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx, seq: Arc::new(AtomicU64::new(0)) }
    }

    pub fn emit(&self, event: Event) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.tx.send(EventEnvelope { seq, at: now_ms(), event });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{ChangeId, Hash, ProjectId};

    #[tokio::test]
    async fn subscribers_get_numbered_events() {
        let events = Events::new();
        let mut rx = events.subscribe();
        events.emit(Event::MainMoved { project: ProjectId::new(), main: Hash::of(b"x"), change: ChangeId::new() });
        let got = rx.recv().await.unwrap();
        assert_eq!(got.seq, 1);
        let json = serde_json::to_value(&got).unwrap();
        assert_eq!(json["event"]["type"], "main.moved");
    }
}
