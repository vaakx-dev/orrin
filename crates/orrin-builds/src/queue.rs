use std::sync::{Arc, Mutex};

use orrin_core::config::BuildsConfig;
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Need {
    pub cpu: u32,
    pub ram_mb: u64,
    pub gpu: u32,
}

impl Need {
    fn fits(&self, used: &Need, total: &Need) -> bool {
        used.cpu + self.cpu <= total.cpu
            && used.ram_mb + self.ram_mb <= total.ram_mb
            && used.gpu + self.gpu <= total.gpu
    }

    fn clamp(self, total: &Need) -> Need {
        Need { cpu: self.cpu.min(total.cpu), ram_mb: self.ram_mb.min(total.ram_mb), gpu: self.gpu.min(total.gpu) }
    }
}

struct Inner {
    total: Need,
    used: Mutex<Need>,
    freed: Notify,
}

pub struct Queue(Arc<Inner>);

pub struct Slot {
    inner: Arc<Inner>,
    need: Need,
}

impl Queue {
    pub fn new(config: &BuildsConfig) -> Self {
        let total = Need {
            cpu: config.cpu.unwrap_or_else(cores).max(1),
            ram_mb: config.ram_mb.unwrap_or(u64::MAX),
            gpu: config.gpu,
        };
        Self(Arc::new(Inner { total, used: Mutex::default(), freed: Notify::new() }))
    }

    pub fn cores(&self) -> u32 {
        self.0.total.cpu
    }

    pub async fn acquire(&self, need: Need) -> Slot {
        let need = need.clamp(&self.0.total);
        loop {
            let freed = self.0.freed.notified();
            tokio::pin!(freed);
            freed.as_mut().enable();
            {
                let mut used = self.0.used.lock().unwrap();
                if need.fits(&used, &self.0.total) {
                    used.cpu += need.cpu;
                    used.ram_mb += need.ram_mb;
                    used.gpu += need.gpu;
                    return Slot { inner: self.0.clone(), need };
                }
            }
            freed.await;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut used = self.inner.used.lock().unwrap();
        used.cpu -= self.need.cpu;
        used.ram_mb -= self.need.ram_mb;
        used.gpu -= self.need.gpu;
        drop(used);
        self.inner.freed.notify_waiters();
    }
}

fn cores() -> u32 {
    std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn waits_for_free_slots() {
        let queue = Queue::new(&BuildsConfig { cpu: Some(2), gpu: 1, ..Default::default() });
        let gpu = Need { cpu: 1, ram_mb: 0, gpu: 1 };
        let first = queue.acquire(gpu).await;
        let second = tokio::time::timeout(Duration::from_millis(50), queue.acquire(gpu)).await;
        assert!(second.is_err());
        drop(first);
        let _again = tokio::time::timeout(Duration::from_millis(50), queue.acquire(gpu)).await.unwrap();
    }
}
