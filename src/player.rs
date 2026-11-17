//! Audio player based on cpal

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicI16, Ordering::Relaxed},
};

use crate::codec::PcmChunk;
use crate::types::SampleFormat;
use cpal::{
    Device, Error, ErrorKind, FromSample, OutputCallbackInfo, SizedSample, StreamConfig,
    SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
// use log::debug;
use std::sync::{Arc, Mutex};

pub struct AudioStream {
    sampleformat: SampleFormat,
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
        for n in 0..chunk.payload.len() / 2 {
            self.data.push_back(i16::from_le_bytes(
                chunk.payload[2 * n..2 * n + 2].try_into().unwrap(),
            ));
        }
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

pub struct Player {
    stream: Option<cpal::Stream>,
    pub volume: Arc<AtomicI16>,
}

impl Player {
    pub fn new() -> Self {
        Player {
            stream: None,
            volume: Arc::new(AtomicI16::new(100)),
        }
    }

    pub fn play(&mut self, audiostream: Arc<Mutex<AudioStream>>) {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");

        let channels: u16;
        let rate: u32;
        {
            let stream = audiostream.lock().unwrap();
            channels = stream.sampleformat.channels;
            rate = stream.sampleformat.rate;
        }

        let config = SupportedStreamConfig::new(
            channels,
            rate,
            *device.default_output_config().unwrap().buffer_size(),
            cpal::SampleFormat::I16,
        );

        self.run::<i16>(
            &device,
            config.into(),
            audiostream,
            Arc::clone(&self.volume),
        )
        .unwrap();
    }

    pub fn run<T>(
        &mut self,
        device: &Device,
        config: StreamConfig,
        audiostream: Arc<Mutex<AudioStream>>,
        volume: Arc<AtomicI16>,
    ) -> Result<(), anyhow::Error>
    where
        T: SizedSample + FromSample<f32>,
    {
        // let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;

        let err_fn = |err: Error| match err.kind() {
            ErrorKind::DeviceChanged | ErrorKind::RealtimeDenied => {
                eprintln!("{err}")
            }
            _ => eprintln!("Stream error: {err}"),
        };

        self.stream = Some(device.build_output_stream(
            config,
            move |data: &mut [T], _info: &OutputCallbackInfo| {
                // if info.xrun() {
                //     eprintln!("output underrun");
                // }
                // if muted, volume is set to -1 * volume
                // debug!(
                //     "Callback ts: {:?}, playback ts: {:?}, diff: {:?}",
                //     _info.timestamp().callback,
                //     _info.timestamp().playback,
                //     _info.timestamp().playback - _info.timestamp().callback
                // );
                let vol: f32 = f32::max(volume.load(Relaxed) as f32 / 100., 0.);
                // debug!("Callback, volume: {}", vol);
                let mut audiostream = audiostream.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    for sample in frame.iter_mut() {
                        let value: T = T::from_sample(
                            audiostream.data.pop_front().unwrap_or(0) as f32 / 32768. * vol,
                        );
                        *sample = value;
                    }
                }
            },
            err_fn,
            None,
        )?);

        Ok(self.stream.as_mut().unwrap().play()?)
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

// fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
// where
//     T: Sample + FromSample<f32>,
// {
//     debug!("write data");
//     for frame in output.chunks_mut(channels) {
//         let value: T = T::from_sample(next_sample());
//         for sample in frame.iter_mut() {
//             *sample = value;
//         }
//     }
// }

// fn write_silence<T>(data: &mut [T], _: &cpal::OutputCallbackInfo)
// where
//     T: Sample + FromSample<f32>,
// {
//     debug!("write silence");
//     for sample in data.iter_mut() {
//         *sample = T::from_sample(rand::random_range(-10000..10000) as f32 / 100.0); // Sample::EQUILIBRIUM;
//     }
// }
