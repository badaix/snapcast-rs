//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::{TimeDiff, TimeSync};
use crate::types::SampleFormat;
use log::trace;
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
        let mut result = Vec::with_capacity(frames as usize);

        if !self.chunks.is_empty() {
            let chunk = self.chunks.front_mut().unwrap();
            let server_now = TimeDiff::now() + self.diff2server;
            let age = server_now - chunk.get_abs_position();
            let playout_time = age + when;
            trace!(
                "get_player_cbunk - playout: {}, frames: {}, when: {:?}, diff to server: {}",
                playout_time, frames, when, self.diff2server
            );
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
            frames_read += chunk.read_frames(frames - frames_read, &mut result);
            trace!("get_player_cbunk - frames read: {}", frames_read);
            if chunk.frames_left() == 0 {
                trace!("get_player_cbunk - pop");
                self.chunks.pop_front();
            }
            if frames_read == frames {
                trace!("get_player_cbunk - frames_read == frames");
                break;
            }
        }
        // trace!("get_player_cbunk - zero: {} - {}", frames_read, frames);
        for _i in frames_read..frames {
            result.push(0.);
            result.push(0.);
        }

        result
    }
}
