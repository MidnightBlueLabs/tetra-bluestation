use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use uuid::Uuid;

/// Minimum playout buffer depth in frames.
const BREW_JITTER_MIN_FRAMES: usize = 2;
/// Default playout buffer depth in frames.
const BREW_JITTER_BASE_FRAMES: usize = 4;
/// Maximum adaptive playout target depth in frames.
const BREW_JITTER_TARGET_MAX_FRAMES: usize = 12;
/// Maximum queued frames kept per call before oldest frames are dropped.
const BREW_JITTER_MAX_FRAMES: usize = 24;
/// Expected receive interval for one TCH/S frame in microseconds (~56.67 ms).
const BREW_EXPECTED_FRAME_INTERVAL_US: f64 = 56_667.0;
/// Warn threshold for excessive adaptive playout depth.
const BREW_JITTER_WARN_TARGET_FRAMES: usize = 8;
/// Rate-limit warning logs per call.
const BREW_JITTER_WARN_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct JitterFrame {
    pub rx_seq: u64,
    pub rx_at: Instant,
    pub acelp_data: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct VoiceJitterBuffer {
    frames: VecDeque<JitterFrame>,
    next_rx_seq: u64,
    started: bool,
    target_frames: usize,
    prev_rx_at: Option<Instant>,
    jitter_us_ewma: f64,
    underrun_boost: usize,
    stable_pops: u32,
    dropped_overflow: u64,
    underruns: u64,
    last_warn_at: Option<Instant>,
    initial_latency_frames: usize,
}

impl VoiceJitterBuffer {
    pub fn with_initial_latency(initial_latency_frames: usize) -> Self {
        let initial = initial_latency_frames.min(BREW_JITTER_TARGET_MAX_FRAMES - BREW_JITTER_MIN_FRAMES);
        Self {
            target_frames: BREW_JITTER_BASE_FRAMES + initial,
            initial_latency_frames: initial,
            ..Default::default()
        }
    }

    pub fn push(&mut self, acelp_data: Vec<u8>) {
        if self.target_frames == 0 {
            self.target_frames = BREW_JITTER_BASE_FRAMES + self.initial_latency_frames;
        }
        let now = Instant::now();
        if let Some(prev) = self.prev_rx_at {
            let delta_us = now.duration_since(prev).as_micros() as f64;
            let deviation_us = (delta_us - BREW_EXPECTED_FRAME_INTERVAL_US).abs();
            self.jitter_us_ewma += (deviation_us - self.jitter_us_ewma) / 16.0;
        }
        self.prev_rx_at = Some(now);

        let frame = JitterFrame {
            rx_seq: self.next_rx_seq,
            rx_at: now,
            acelp_data,
        };
        self.next_rx_seq = self.next_rx_seq.wrapping_add(1);
        self.frames.push_back(frame);
        while self.frames.len() > BREW_JITTER_MAX_FRAMES {
            self.frames.pop_front();
            self.dropped_overflow += 1;
        }
        self.recompute_target();
    }

    pub fn pop_ready(&mut self) -> Option<JitterFrame> {
        if self.target_frames == 0 {
            self.target_frames = BREW_JITTER_BASE_FRAMES + self.initial_latency_frames;
        }

        if !self.started {
            if self.frames.len() < self.target_frames {
                return None;
            }
            self.started = true;
        }

        match self.frames.pop_front() {
            Some(frame) => {
                if self.frames.len() >= self.target_frames {
                    self.stable_pops = self.stable_pops.saturating_add(1);
                    if self.stable_pops >= 80 {
                        self.stable_pops = 0;
                        if self.underrun_boost > 0 {
                            self.underrun_boost -= 1;
                            self.recompute_target();
                        }
                    }
                } else {
                    self.stable_pops = 0;
                }
                Some(frame)
            }
            None => {
                self.started = false;
                self.underruns += 1;
                self.underrun_boost = (self.underrun_boost + 1).min(4);
                self.stable_pops = 0;
                self.recompute_target();
                None
            }
        }
    }

    pub fn target_frames(&self) -> usize {
        self.target_frames.max(BREW_JITTER_MIN_FRAMES)
    }

    fn recompute_target(&mut self) {
        let jitter_component = ((self.jitter_us_ewma * 2.0) / BREW_EXPECTED_FRAME_INTERVAL_US).ceil() as usize;
        let target = BREW_JITTER_BASE_FRAMES + self.initial_latency_frames + jitter_component + self.underrun_boost;
        self.target_frames = target.clamp(BREW_JITTER_MIN_FRAMES, BREW_JITTER_TARGET_MAX_FRAMES);
    }

    pub fn maybe_warn_unhealthy(&mut self, uuid: Uuid) {
        let now = Instant::now();
        if let Some(last_warn) = self.last_warn_at {
            if now.duration_since(last_warn) < BREW_JITTER_WARN_INTERVAL {
                return;
            }
        }

        if self.target_frames() < BREW_JITTER_WARN_TARGET_FRAMES && self.underruns == 0 {
            return;
        }

        self.last_warn_at = Some(now);
        tracing::warn!(
            "BrewEntity: high jitter on uuid={} target_frames={} queue={} underruns={} overflow_drops={} jitter_ms={:.1}",
            uuid,
            self.target_frames(),
            self.frames.len(),
            self.underruns,
            self.dropped_overflow,
            self.jitter_us_ewma / 1000.0
        );
    }
}
