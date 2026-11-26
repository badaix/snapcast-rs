//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::{TimeDiff, TimeSync};
use crate::types::{SampleFormat, TimeStamp};
use log::{info, trace};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Statistics {
    diffs: VecDeque<(i64, u32)>,
    last_log: TimeStamp,
    median: Option<i64>,
    samples: u32,
    diff_frames: i32,
    sampleformat: SampleFormat,
    frames_to_add: i32,
}

impl Statistics {
    pub fn new(sampleformat: SampleFormat) -> Self {
        Statistics {
            diffs: VecDeque::new(),
            last_log: TimeStamp::now(),
            median: None,
            samples: 0,
            diff_frames: 0,
            sampleformat,
            frames_to_add: 0,
        }
    }

    pub fn clear(&mut self) {
        self.diffs.clear();
        self.median = None;
    }

    pub fn add(&mut self, diff: (TimeDiff, u32)) {
        self.diffs.push_back((diff.0.as_micros(), diff.1));
        self.samples = self.diffs.iter().map(|n| n.1).sum::<u32>();
        while !self.diffs.is_empty() && self.samples > self.sampleformat.rate + diff.1 {
            self.samples -= self.diffs.pop_front().unwrap_or((0, 0)).1;
        }
        self.calc_median();
    }

    fn calc_median(&mut self) {
        let now = TimeStamp::now();
        if now - self.last_log > TimeDiff::new(1, 0) && !self.diffs.is_empty() {
            self.last_log = now;
            let mut v = Vec::from(self.diffs.clone());
            v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            self.median = Some(v[v.len() / 2].0);
            self.diff_frames =
                (self.median.unwrap() as i32 * self.sampleformat.rate as i32) / 1_000_000;
            self.samples = self.diffs.iter().map(|n| n.1).sum::<u32>();
            if self.median.unwrap().unsigned_abs() / 100 > 1 {
                self.frames_to_add = self.diff_frames;
            } else {
                self.frames_to_add = 0
            }
            info!(target: "stats", "Chunk: {} \tdiff frames: {} \tsamples: {} \tx: {}", self.median.unwrap() / 100, self.diff_frames, v.len(), self.samples);
        }
    }

    pub fn get_diff(&mut self) -> Option<i64> {
        self.calc_median();
        if self.samples >= self.sampleformat.rate && self.median.is_some() {
            self.median
        } else {
            None
        }
    }

    pub fn frames_to_add(&mut self) -> i32 {
        let result = self.frames_to_add;
        self.frames_to_add = 0;
        result
    }
}

pub struct AudioStream {
    pub sampleformat: SampleFormat,
    chunks: VecDeque<PcmChunk>,
    timesync: Arc<Mutex<TimeSync>>,
    diff2server: TimeDiff,
    stats: Statistics,
}

impl AudioStream {
    pub fn new(sampleformat: SampleFormat, timesync: Arc<Mutex<TimeSync>>) -> Self {
        AudioStream {
            sampleformat,
            chunks: VecDeque::new(),
            timesync,
            diff2server: TimeDiff::new(0, 0),
            stats: Statistics::new(sampleformat),
        }
    }

    pub fn add(&mut self, chunk: PcmChunk) {
        self.chunks.push_back(chunk);
    }

    // fn get_age(&self, chunk: &PcmChunk) -> TimeDiff {
    //     let server_now = TimeDiff::now() + self.diff2server;
    //     server_now - chunk.timestamp + chunk.get_position()
    // }

    pub fn get_player_cbunk(&mut self, when: Duration, frames: u32) -> Vec<f32> {
        {
            self.diff2server = self.timesync.lock().unwrap().get_diff();
        }
        let mut result = Vec::with_capacity(frames as usize * self.sampleformat.channels as usize);
        let mut to_read = frames;

        if !self.chunks.is_empty() {
            let chunk = self.chunks.front_mut().unwrap();
            let server_now = TimeDiff::now() + self.diff2server;
            let age = server_now - chunk.get_abs_position();
            let playout_time = age + when;

            let buffer_duration = TimeDiff::from_duration(Duration::from_millis(1000));
            let delta = playout_time - buffer_duration;
            self.stats.add((delta, frames));
            let frames_delta =
                (delta.as_micros() as f32 * self.sampleformat.rate as f32 / 1_000_000_f32) as isize;
            trace!(
                "get_player_cbunk - playout: {}, frames: {}, when: {:?}, diff to server: {}, delta: {}",
                playout_time,
                frames,
                when,
                self.diff2server,
                delta.as_millis()
            );
            let threshold_ms = 5;

            // Check if we are elary and need to play some silence while wating for "old enough" chunks
            if delta.as_millis() < -threshold_ms
            // || self.stats.get_diff().unwrap_or(0) / 1000 < threshold_ms
            {
                // Fill up with zeros up to the requested size, should be a noop if we had enough chunks
                info!(
                    "Delta < {}ms: {:?} ({} frames), playing silence",
                    -threshold_ms, delta, frames_delta
                );
                let frames_delta = frames_delta.unsigned_abs();
                if frames_delta < frames as usize {
                    result.resize(frames_delta * self.sampleformat.channels as usize, 0_f32);
                    to_read = (frames as i32 - frames_delta as i32) as u32;
                } else {
                    result.resize(frames as usize * self.sampleformat.channels as usize, 0_f32);
                    return result;
                }
                self.stats.clear();
            }
            // Check if we are late and need to drop some frames
            else if delta.as_millis() > threshold_ms {
                let mut skipped = 0;
                while !self.chunks.is_empty() {
                    let chunk = self.chunks.front_mut().unwrap();
                    skipped += chunk.skip_frames(frames - skipped);
                    info!("Skipped {} of {} frames", skipped, frames);
                    if chunk.frames_left() == 0 {
                        self.chunks.pop_front();
                    }
                    if skipped == frames {
                        break;
                    }
                }
                self.stats.clear();
            }
        }

        // TODO: timesync, optimize, ...
        let mut frames_read = 0_u32;
        while !self.chunks.is_empty() {
            trace!("get_player_cbunks: {}", self.chunks.len());

            let chunk = self.chunks.front_mut().unwrap();
            let server_now = TimeDiff::now() + self.diff2server;
            let age = server_now - chunk.timestamp + chunk.get_position();
            trace!(
                "get_player_cbunk - chunk len: {}, left: {}, age: {:?}",
                chunk.frame_count(),
                chunk.frames_left(),
                age
            );
            let frames_to_add = self.stats.frames_to_add();
            if frames_to_add > 0 {
                info!("frames_to_add > 0, skipping frame");
                chunk.skip_frames(1);
            } else if frames_to_add < 0 {
                info!("frames_to_add < 0, duplicating frame");
                frames_read += chunk.read_frames(1, &mut result);
                result.push(*result.last().unwrap());
                frames_read += 1;
            }
            frames_read += chunk.read_frames(to_read - frames_read, &mut result);
            trace!("get_player_cbunk - frames read: {}", frames_read);
            if chunk.frames_left() == 0 {
                trace!("get_player_cbunk - pop");
                self.chunks.pop_front();
            }
            if frames_read == to_read {
                trace!("get_player_cbunk - frames_read == frames");
                break;
            }
        }

        // Fill up with zeros up to the requested size, should be a noop if we had enough chunks
        result.resize(frames as usize * self.sampleformat.channels as usize, 0_f32);
        result
    }
}
