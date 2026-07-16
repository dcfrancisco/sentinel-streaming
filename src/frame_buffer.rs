use crate::frame::Frame;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    RwLock,
};
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone)]
pub struct FrameBuffer {
    capacity: usize,
    frames: Arc<RwLock<VecDeque<Arc<Frame>>>>,
    evictions: Arc<AtomicU64>,
}
impl FrameBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "frame buffer capacity must be greater than zero"
        );
        Self {
            capacity,
            frames: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            evictions: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn push(&self, frame: Frame) -> Arc<Frame> {
        let frame = Arc::new(frame);
        let mut frames = self.frames.write().expect("frame buffer lock poisoned");
        if frames.len() == self.capacity {
            frames.pop_front();
            self.evictions.fetch_add(1, Ordering::Relaxed);
        }
        frames.push_back(frame.clone());
        frame
    }
    pub fn latest(&self) -> Option<Arc<Frame>> {
        self.frames
            .read()
            .expect("frame buffer lock poisoned")
            .back()
            .cloned()
    }
    /// Returns the second-most-recent frame for temporal consumers.
    #[allow(dead_code)]
    pub fn previous(&self) -> Option<Arc<Frame>> {
        self.frames
            .read()
            .expect("frame buffer lock poisoned")
            .iter()
            .rev()
            .nth(1)
            .cloned()
    }
    /// Looks up a retained frame by its capture sequence.
    #[allow(dead_code)]
    pub fn by_sequence(&self, sequence: u64) -> Option<Arc<Frame>> {
        self.frames
            .read()
            .expect("frame buffer lock poisoned")
            .iter()
            .find(|frame| frame.sequence == sequence)
            .cloned()
    }
    pub fn recent(&self, count: usize) -> Vec<Arc<Frame>> {
        self.frames
            .read()
            .expect("frame buffer lock poisoned")
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }
    pub fn len(&self) -> usize {
        self.frames
            .read()
            .expect("frame buffer lock poisoned")
            .len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }
    pub fn utilization(&self) -> f64 {
        self.len() as f64 / self.capacity as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evicts_oldest_frame_at_capacity() {
        let buffer = FrameBuffer::new(2);
        buffer.push(Frame::blank(1, 1, 1));
        buffer.push(Frame::blank(2, 1, 1));
        buffer.push(Frame::blank(3, 1, 1));
        assert!(buffer.by_sequence(1).is_none());
        assert_eq!(buffer.latest().unwrap().sequence, 3);
        assert_eq!(
            buffer
                .recent(2)
                .iter()
                .map(|frame| frame.sequence)
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert_eq!(buffer.previous().unwrap().sequence, 2);
        assert_eq!(buffer.by_sequence(2).unwrap().sequence, 2);
        assert_eq!(buffer.evictions(), 1);
        assert!((buffer.utilization() - 1.0).abs() < f64::EPSILON);
    }
}
