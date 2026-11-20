//! Time synchronized audio stream

use crate::codec::PcmChunk;
use crate::timesync::TimeDiff;
use crate::types::SampleFormat;
use std::collections::VecDeque;

pub struct AudioStream {
    pub sampleformat: SampleFormat,
    // chunks: VecDeque<PcmChunk>,
    data: VecDeque<i16>,
}

impl AudioStream {
    pub fn new(sampleformat: SampleFormat) -> Self {
        AudioStream {
            sampleformat,
            data: VecDeque::new(),
        }
    }

    pub fn add(&mut self, chunk: PcmChunk) {
        // self.chunks.push_back(chunk);
        for n in 0..chunk.payload.len() / 2 {
            self.data.push_back(i16::from_le_bytes(
                chunk.payload[2 * n..2 * n + 2].try_into().unwrap(),
            ));
        }
    }

    pub fn get_player_cbunk(&mut self, _when: &TimeDiff, frames: u32) -> Vec<f32> {
        let mut result = Vec::new();
        for _i in 0..frames {
            result.push(self.data.pop_front().unwrap_or(0) as f32 / 32768.);
        }
        result
    }

    pub fn fetch_data(&mut self) {
        // while let Ok(chunk) = self.rx.try_recv() {
        //     // debug!("Received chunk: {}", chunk.payload.len());
        //     for n in 0..chunk.payload.len() / 2 {
        //         self.data.push_back(i16::from_le_bytes(
        //             chunk.payload[2 * n..2 * n + 2].try_into().unwrap(),
        //         ));
        //     }
        //     // debug!("data: {}", self.data.len());
        // }
    }
}
