//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::{TimeDiff, TimeSync};
use crate::types::SampleFormat;
use log::{info, trace};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AudioStream {
    pub sampleformat: SampleFormat,
    chunks: VecDeque<PcmChunk>,
    timesync: Arc<Mutex<TimeSync>>,
    diff2server: TimeDiff,
}

impl AudioStream {
    pub fn new(sampleformat: SampleFormat, timesync: Arc<Mutex<TimeSync>>) -> Self {
        AudioStream {
            sampleformat,
            chunks: VecDeque::new(),
            timesync,
            diff2server: TimeDiff::new(0, 0),
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
            let delta = (buffer_duration - playout_time)
                .to_duration()
                .unwrap_or(Duration::from_millis(0));
            let frames_delta =
                (delta.as_micros() as f32 * self.sampleformat.rate as f32 / 1_000_000_f32) as usize;
            if delta > Duration::from_millis(5) {
                // Fill up with zeros up to the requested size, should be a noop if we had enough chunks
                info!(
                    "Delta > 5ms: {:?} ({} frames), playing silence",
                    delta, frames_delta
                );
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
