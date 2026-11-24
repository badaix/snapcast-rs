//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::{TimeDiff, TimeSync};
use crate::types::{SampleFormat, TimeStamp};
use log::{info, trace};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Statistics {
    diffs: VecDeque<i32>,
    last_log: TimeStamp,
}

impl Default for Statistics {
    fn default() -> Self {
        Self::new()
    }
}

impl Statistics {
    pub fn new() -> Self {
        Statistics {
            diffs: VecDeque::new(),
            last_log: TimeStamp::now(),
        }
    }

    pub fn add(&mut self, diff: TimeDiff) {
        self.diffs
            .push_back(diff.sec as i32 * 10_000 + diff.usec / 100);
        if self.diffs.len() > 50 {
            self.diffs.pop_front();
        }
    }

    pub fn log(&mut self) {
        let now = TimeStamp::now();
        if now - self.last_log > TimeDiff::new(1, 0) && !self.diffs.is_empty() {
            self.last_log = now;
            // let sum = self.diffs[9] + self.diffs[2];
            let mut v = Vec::from(self.diffs.clone());
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = v[v.len() / 2];
            // let mean = self.diffs.iter().sum::<i32>() / self.diffs.len() as i32;
            info!(target: "stats", "Stats - Chunk: {}, samples: {}", median, v.len());
        }
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
            stats: Statistics::new(),
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
            trace!(
                "get_player_cbunk - playout: {}, frames: {}, when: {:?}, diff to server: {}",
                playout_time, frames, when, self.diff2server
            );

            // Check if we are elary and need to play some silence while wating for "old enough" chunks
            let buffer_duration = TimeDiff::from_duration(Duration::from_millis(1000));
            let delta = playout_time - buffer_duration;
            let frames_delta =
                (delta.as_micros() as f32 * self.sampleformat.rate as f32 / 1_000_000_f32) as isize;
            let threshold_ms = -5;
            if delta.as_millis() < threshold_ms {
                // Fill up with zeros up to the requested size, should be a noop if we had enough chunks
                info!(
                    "Delta < {}ms: {:?} ({} frames), playing silence",
                    threshold_ms, delta, frames_delta
                );
                let frames_delta = frames_delta.unsigned_abs();
                if frames_delta < frames as usize {
                    result.resize(frames_delta * self.sampleformat.channels as usize, 0_f32);
                    to_read = (frames as i32 - frames_delta as i32) as u32;
                } else {
                    result.resize(frames as usize * self.sampleformat.channels as usize, 0_f32);
                    return result;
                }

                // while !self.chunks.is_empty() {
                //     debug!("Left: {:?}", self.chunks.front().unwrap().get_left());
                //     if self.chunks.front().unwrap().get_left() > delta {
                //         self.chunks.pop_front();
                //         continue;
                //     }
                // }
            }
            self.stats.add(buffer_duration - playout_time);
            self.stats.log();
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
