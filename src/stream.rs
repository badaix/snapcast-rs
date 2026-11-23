//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::{TimeDiff, TimeSync};
use crate::types::SampleFormat;
use log::{debug, trace};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AudioStream {
    pub sampleformat: SampleFormat,
    chunks: VecDeque<PcmChunk>,
    timesync: Arc<Mutex<TimeSync>>,
    diff2server: TimeDiff,
    last_callback: TimeDiff,
}

impl AudioStream {
    pub fn new(sampleformat: SampleFormat, timesync: Arc<Mutex<TimeSync>>) -> Self {
        AudioStream {
            sampleformat,
            chunks: VecDeque::new(),
            timesync,
            diff2server: TimeDiff::new(0, 0),
            last_callback: TimeDiff::new(0, 0),
        }
    }

    pub fn add(&mut self, chunk: PcmChunk) {
        self.chunks.push_back(chunk);
    }

    // fn frames_left(&self, chunk: &PcmChunk) -> u16 {
    //     self.frame_count(chunk) - self.frame_pos(chunk)
    // }

    // fn frame_pos(&self, chunk: &PcmChunk) -> u16 {
    //     chunk.cursor as u16 / self.get_framesize()
    // }

    // fn frame_count(&self, chunk: &PcmChunk) -> u16 {
    //     chunk.payload.len() as u16 / self.get_framesize()
    // }

    // fn get_duration(&self, chunk: &PcmChunk) -> Duration {
    //     let ms = self.frame_count(chunk) as f32 / (self.sampleformat.rate as f32 / 1000.);
    //     Duration::from_micros((ms * 1000.) as u64)
    // }

    // fn get_position(&self, chunk: &PcmChunk) -> Duration {
    //     let ms = self.frame_pos(chunk) as f32 / (self.sampleformat.rate as f32 / 1000.);
    //     Duration::from_micros((ms * 1000.) as u64)
    // }

    // fn get_left(&self, chunk: &PcmChunk) -> Duration {
    //     self.get_duration(chunk) - self.get_position(chunk)
    // }

    // fn get_framesize(&self) -> u16 {
    //     (self.sampleformat.bits / 8) * self.sampleformat.channels
    // }

    // fn read_frames(&self, chunk: &mut PcmChunk, frames: u32, data: &mut Vec<f32>) -> u32 {
    //     let samples = cmp::min(frames as u16, self.frames_left(chunk)) as usize
    //         * self.sampleformat.channels as usize;
    //     for i in chunk.cursor..chunk.cursor + samples {
    //         let ival = i16::from_le_bytes(
    //             chunk.payload[2 * i + chunk.cursor..2 * i + chunk.cursor + 2]
    //                 .try_into()
    //                 .unwrap(),
    //         );
    //         data.push(ival as f32);
    //         chunk.cursor += 2 * samples;
    //     }
    //     samples as u32 / self.sampleformat.channels as u32
    // }

    // fn get_age(&self, chunk: &PcmChunk) -> TimeDiff {
    //     let server_now = TimeDiff::now() + self.diff2server;
    //     server_now - chunk.timestamp + chunk.get_position()
    // }

    pub fn get_player_cbunk(&mut self, when: Duration, frames: u32) -> Vec<f32> {
        {
            self.diff2server = self.timesync.lock().unwrap().get_diff();
        }
        trace!(
            "get_player_cbunk - frames: {}, when: {:?}, diff to server: {:?}",
            frames, when, self.diff2server
        );
        let mut result = Vec::new();

        if !self.chunks.is_empty() {
            let chunk = self.chunks.front_mut().unwrap();
            let server_now = TimeDiff::now() + self.diff2server;
            let age = server_now - chunk.timestamp + chunk.get_position();
            let playout_time = age + when;
            let chunk_ts = chunk.timestamp;
            let frames_dur = Duration::from_micros(
                (frames as f32 / self.sampleformat.rate as f32 * 1000. * 1000.) as u64,
            );
            let now = TimeDiff::now();
            debug!(
                "get_player_cbunk - playout: {}, now: {}, diff: {}, frames dur: {:?}, now - frames dur: {}, age: {}, chunk ts: {}, pos: {:?}, server_now: {}, when: {:?}, frames: {}, ts: {}",
                playout_time,
                now,
                now - self.last_callback,
                frames_dur,
                now - frames_dur,
                age,
                chunk_ts,
                chunk.get_position(),
                server_now,
                when,
                frames,
                chunk.timestamp
            );
            self.last_callback = now;
        }

        // TODO: timesync, optimize, ...
        let mut frames_read = 0_u32;
        while !self.chunks.is_empty() {
            debug!("get_player_cbunks: {}", self.chunks.len());

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
                debug!("get_player_cbunk - pop");
                self.chunks.pop_front();
            }
            if frames_read == frames {
                trace!("get_player_cbunk - frames_read == frames");
                break;
            }
            // if chunk.frames_left() > 0 {
            //     self.chunks.push_front(chunk);
            // }
        }
        // trace!("get_player_cbunk - zero: {} - {}", frames_read, frames);
        for _i in frames_read..frames {
            result.push(0.);
            result.push(0.);
        }

        // for _i in 0..samples {
        //     let mut val: f32 = 0.;
        //     if !self.chunks.is_empty() {
        //         let chunk = self.chunks.front_mut().unwrap();
        //         let ival = i16::from_le_bytes(
        //             chunk.payload[chunk.cursor..chunk.cursor + 2]
        //                 .try_into()
        //                 .unwrap(),
        //         );
        //         if chunk.cursor + 2 >= chunk.payload.len() {
        //             if log::log_enabled!(log::Level::Trace) {
        //                 let buffer_duration = TimeDiff::new(1, 0);
        //                 let server_now = TimeDiff::now() + self.diff2server;
        //                 let chunk_age = server_now - chunk.timestamp;
        //                 let playout_time = buffer_duration - chunk_age;
        //                 // trace!(
        //                 //     "chunk buffer: {:?}, server now: {:?}, chunk age: {:?}, playout time: {:?}, age: {:?}",
        //                 //     buffer_duration,
        //                 //     server_now,
        //                 //     chunk_age,
        //                 //     playout_time,
        //                 //     self.get_age(&chunk)
        //                 // );
        //             }
        //             self.chunks.pop_front();
        //         } else {
        //             chunk.cursor += 2;
        //         }
        //         val = ival as f32 / 32768.;
        //     }
        //     result.push(val);
        // }

        result
    }
}
