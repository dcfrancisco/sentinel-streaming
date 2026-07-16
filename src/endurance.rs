use crate::{
    events::{EventRecord, EventStore},
    frame::Frame,
    frame_buffer::FrameBuffer,
};
use anyhow::{anyhow, Context, Result};
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};
use serde::Serialize;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct EnduranceOptions {
    pub duration: Duration,
    pub viewers: usize,
    pub vision_mock: bool,
    pub report: Option<PathBuf>,
    pub min_fps: f64,
}

#[derive(Debug, Serialize)]
pub struct EnduranceReport {
    pub duration_seconds: f64,
    pub frames_captured: u64,
    pub frames_streamed: u64,
    pub active_viewers: usize,
    pub vision_successes: u64,
    pub vision_failures: u64,
    pub event_count: usize,
    pub buffer_size: usize,
    pub buffer_capacity: usize,
    pub buffer_utilization: f64,
    pub frame_evictions: u64,
    pub stream_bytes: u64,
    pub capture_fps: f64,
    pub stream_fps: f64,
    pub errors: u64,
    pub passed: bool,
    pub failures: Vec<String>,
}

pub async fn run(options: EnduranceOptions) -> Result<EnduranceReport> {
    if options.duration.is_zero() {
        return Err(anyhow!("endurance duration must be greater than zero"));
    }
    let buffer = FrameBuffer::new(300);
    let events = EventStore::new(1000);
    let started = Instant::now();
    let deadline = started + options.duration;
    let mut frames_captured = 0;
    let mut frames_streamed = 0;
    let mut stream_bytes = 0;
    let mut vision_successes = 0;
    let mut errors = 0;
    let mut sequence = 0;
    let mut ticker = tokio::time::interval(Duration::from_millis(33));
    while Instant::now() < deadline {
        ticker.tick().await;
        sequence += 1;
        buffer.push(Frame::blank(sequence, 64, 48));
        frames_captured += 1;
        if let Some(frame) = buffer.latest() {
            let mut jpeg = Vec::new();
            if JpegEncoder::new(&mut jpeg)
                .encode(
                    frame.data.as_ref(),
                    frame.width,
                    frame.height,
                    ExtendedColorType::Rgb8,
                )
                .is_err()
            {
                errors += 1;
            } else {
                frames_streamed += options.viewers as u64;
                stream_bytes += jpeg.len() as u64 * options.viewers as u64;
            }
        }
        if options.vision_mock && sequence % 30 == 0 {
            vision_successes += 1;
            events
                .push(EventRecord::simple(
                    "scene.observed",
                    Some("synthetic".into()),
                    "synthetic scene observed",
                ))
                .await;
        }
    }
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let capture_fps = frames_captured as f64 / elapsed;
    let stream_fps = if options.viewers == 0 {
        0.0
    } else {
        frames_streamed as f64 / options.viewers as f64 / elapsed
    };
    let mut failures = Vec::new();
    if capture_fps < options.min_fps {
        failures.push(format!(
            "capture FPS {capture_fps:.2} below minimum {:.2}",
            options.min_fps
        ));
    }
    if errors > 0 {
        failures.push(format!("{errors} encoding errors"));
    }
    let report = EnduranceReport {
        duration_seconds: elapsed,
        frames_captured,
        frames_streamed,
        active_viewers: options.viewers,
        vision_successes,
        vision_failures: 0,
        event_count: events.len().await,
        buffer_size: buffer.len(),
        buffer_capacity: buffer.capacity(),
        buffer_utilization: buffer.utilization(),
        frame_evictions: buffer.evictions(),
        stream_bytes,
        capture_fps,
        stream_fps,
        errors,
        passed: failures.is_empty(),
        failures,
    };
    if let Some(path) = options.report {
        let json = serde_json::to_vec_pretty(&report).context("serialize endurance report")?;
        std::fs::write(&path, json)
            .with_context(|| format!("write endurance report {}", path.display()))?;
    }
    println!(
        "endurance: {:.2} capture FPS, {:.2} stream FPS, {} frames, {} viewers, {}",
        report.capture_fps,
        report.stream_fps,
        report.frames_captured,
        report.active_viewers,
        if report.passed { "PASS" } else { "FAIL" }
    );
    if report.passed {
        Ok(report)
    } else {
        Err(anyhow!(report.failures.join("; ")))
    }
}

pub fn parse_duration(value: &str) -> Result<Duration> {
    let (number, unit) = value.split_at(value.len().saturating_sub(1));
    let amount: u64 = number
        .parse()
        .with_context(|| format!("invalid duration '{value}'"))?;
    match unit {
        "s" => Ok(Duration::from_secs(amount)),
        "m" => Ok(Duration::from_secs(amount * 60)),
        "h" => Ok(Duration::from_secs(amount * 60 * 60)),
        _ => Err(anyhow!("duration must end in s, m, or h")),
    }
}
