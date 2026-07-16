use serde::Serialize;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Instant;

#[derive(Clone)]
pub struct Metrics {
    started: Instant,
    frames: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    sources: Arc<AtomicU64>,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub uptime_seconds: u64,
    pub connected_sources: u64,
    pub fps: f64,
    pub dropped_frames: u64,
    pub memory_bytes: u64,
}
impl Metrics {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            frames: Arc::new(AtomicU64::new(0)),
            dropped: Arc::new(AtomicU64::new(0)),
            sources: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn connected(&self, n: u64) {
        self.sources.store(n, Ordering::Relaxed);
    }
    pub fn frame(&self) {
        self.frames.fetch_add(1, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> Snapshot {
        let uptime = self.started.elapsed().as_secs();
        Snapshot {
            uptime_seconds: uptime,
            connected_sources: self.sources.load(Ordering::Relaxed),
            fps: if uptime == 0 {
                0.0
            } else {
                self.frames.load(Ordering::Relaxed) as f64 / uptime as f64
            },
            dropped_frames: self.dropped.load(Ordering::Relaxed),
            memory_bytes: memory_bytes(),
        }
    }
    pub fn prometheus(&self) -> String {
        let s = self.snapshot();
        format!("# HELP sentinel_uptime_seconds Process uptime.\n# TYPE sentinel_uptime_seconds gauge\nsentinel_uptime_seconds {}\nsentinel_connected_sources {}\nsentinel_fps {}\nsentinel_dropped_frames {}\nsentinel_memory_bytes {}\n", s.uptime_seconds, s.connected_sources, s.fps, s.dropped_frames, s.memory_bytes)
    }
}
fn memory_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            .unwrap_or(0)
            * 4096
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}
