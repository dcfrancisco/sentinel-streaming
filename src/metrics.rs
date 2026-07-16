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
    started_cpu_us: u64,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub uptime_seconds: u64,
    pub connected_sources: u64,
    pub fps: f64,
    pub dropped_frames: u64,
    pub memory_bytes: u64,
    pub cpu_percent: f64,
}
impl Metrics {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            frames: Arc::new(AtomicU64::new(0)),
            dropped: Arc::new(AtomicU64::new(0)),
            sources: Arc::new(AtomicU64::new(0)),
            started_cpu_us: process_cpu_time_us(),
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
            cpu_percent: cpu_percent(self.started, self.started_cpu_us),
        }
    }
    pub fn prometheus(&self) -> String {
        let s = self.snapshot();
        format!("# HELP sentinel_uptime_seconds Process uptime.\n# TYPE sentinel_uptime_seconds gauge\nsentinel_uptime_seconds {}\nsentinel_connected_sources {}\nsentinel_fps {}\nsentinel_dropped_frames {}\nsentinel_memory_bytes {}\nsentinel_cpu_percent {}\n", s.uptime_seconds, s.connected_sources, s.fps, s.dropped_frames, s.memory_bytes, s.cpu_percent)
    }
}
impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
fn memory_bytes() -> u64 {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
            let usage = unsafe { usage.assume_init() };
            #[cfg(target_os = "macos")]
            let multiplier = 1;
            #[cfg(not(target_os = "macos"))]
            let multiplier = 1024;
            return usage.ru_maxrss as u64 * multiplier;
        }
        0
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn process_cpu_time_us() -> u64 {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
            let usage = unsafe { usage.assume_init() };
            (usage.ru_utime.tv_sec as u64 * 1_000_000 + usage.ru_utime.tv_usec as u64)
                + (usage.ru_stime.tv_sec as u64 * 1_000_000 + usage.ru_stime.tv_usec as u64)
        } else {
            0
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn cpu_percent(started: Instant, started_cpu_us: u64) -> f64 {
    let elapsed_us = started.elapsed().as_micros() as u64;
    if elapsed_us == 0 {
        return 0.0;
    }
    process_cpu_time_us().saturating_sub(started_cpu_us) as f64 / elapsed_us as f64 * 100.0
}
