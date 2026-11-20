//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::{TimeDiff, TimeSync};
use crate::types::SampleFormat;
use log::trace;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct AudioStream {
    pub sampleformat: SampleFormat,
    chunks: VecDeque<PcmChunk>,
    cursor: usize,
    timesync: Arc<Mutex<TimeSync>>,
    diff2server: TimeDiff,
}

impl AudioStream {
    pub fn new(sampleformat: SampleFormat, timesync: Arc<Mutex<TimeSync>>) -> Self {
        AudioStream {
            sampleformat,
            chunks: VecDeque::new(),
            cursor: 0,
            timesync,
            diff2server: TimeDiff::new(0, 0),
        }
    }

    pub fn add(&mut self, chunk: PcmChunk) {
        self.chunks.push_back(chunk);
    }

    // fn get_age(&mut self) -> Option<TimeDiff> {
    //     if self.chunks.is_empty() {
    //         None
    //     }

    //     let server_now = TimeDiff::now() + self.diff2server;
    //     let chunk_age = server_now - self.chunks.front().unwrap().timestamp;

    //     self.cursor
    //     let playout_time = buffer_duration - chunk_age;
    //     trace!(
    //         "chunk buffer: {:?}, server now: {:?}, chunk age: {:?}, playout time: {:?}",
    //         buffer_duration, server_now, chunk_age, playout_time
    //     );
    // }

    pub fn get_player_cbunk(&mut self, when: &TimeDiff, frames: u32) -> Vec<f32> {
        {
            self.diff2server = self.timesync.lock().unwrap().get_diff();
        }
        trace!(
            "get_player_cbunk - when: {:?}, diff to server: {:?}",
            when, self.diff2server
        );
        let mut result = Vec::new();

        // TODO: timesync, optimize, ...
        let samples = frames * self.sampleformat.channels as u32;
        for _i in 0..samples {
            let mut val: f32 = 0.;
            if !self.chunks.is_empty() {
                let chunk = self.chunks.front().unwrap();
                let ival = i16::from_le_bytes(
                    chunk.payload[self.cursor..self.cursor + 2]
                        .try_into()
                        .unwrap(),
                );
                if self.cursor + 2 >= chunk.payload.len() {
                    if log::log_enabled!(log::Level::Trace) {
                        let buffer_duration = TimeDiff::new(1, 0);
                        let server_now = TimeDiff::now() + self.diff2server;
                        let chunk_age = server_now - chunk.timestamp;
                        let playout_time = buffer_duration - chunk_age;
                        trace!(
                            "chunk buffer: {:?}, server now: {:?}, chunk age: {:?}, playout time: {:?}",
                            buffer_duration, server_now, chunk_age, playout_time
                        );
                    }
                    self.chunks.pop_front();
                    self.cursor = 0;
                } else {
                    self.cursor += 2;
                }
                val = ival as f32 / 32768.;
            }
            result.push(val);
        }

        result
    }
}
