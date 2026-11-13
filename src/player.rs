//! Audio player based on cpal

use std::collections::VecDeque;

use cpal::{
    Device, Error, ErrorKind, FromSample, I24, OutputCallbackInfo, SampleFormat, SizedSample,
    StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use log::debug;
use tokio::sync::mpsc;

use crate::codec;

pub struct Stream {
    rx: mpsc::Receiver<codec::PcmChunk>,
    data: VecDeque<i16>,
}

impl Stream {
    pub fn new(rx: mpsc::Receiver<codec::PcmChunk>) -> Self {
        Stream {
            rx,
            data: VecDeque::new(),
        }
    }

    pub fn fetch_data(&mut self) {
        while let Ok(chunk) = self.rx.try_recv() {
            // debug!("Received chunk: {}", chunk.payload.len());
            for n in 0..chunk.payload.len() / 2 {
                self.data.push_back(i16::from_le_bytes(
                    chunk.payload[2 * n..2 * n + 2].try_into().unwrap(),
                ));
            }
            // debug!("data: {}", self.data.len());
        }
    }
}

pub struct Player {
    stream: Option<cpal::Stream>,
}

impl Player {
    pub fn new() -> Self {
        Player { stream: None }
    }

    pub fn play(&mut self, stream: Stream) {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");

        let config = device.default_output_config().unwrap();
        debug!("Default output config: {config:?}");
        match config.sample_format() {
            SampleFormat::I8 => self.run::<i8>(&device, config.into(), stream),
            SampleFormat::I16 => self.run::<i16>(&device, config.into(), stream),
            SampleFormat::I24 => self.run::<I24>(&device, config.into(), stream),
            SampleFormat::I32 => self.run::<i32>(&device, config.into(), stream),
            // SampleFormat::I48 => self.run::<I48>(&device, config.into()),
            SampleFormat::I64 => self.run::<i64>(&device, config.into(), stream),
            SampleFormat::U8 => self.run::<u8>(&device, config.into(), stream),
            SampleFormat::U16 => self.run::<u16>(&device, config.into(), stream),
            // SampleFormat::U24 => self.run::<U24>(&device, config.into()),
            SampleFormat::U32 => self.run::<u32>(&device, config.into(), stream),
            // SampleFormat::U48 => self.run::<U48>(&device, config.into()),
            SampleFormat::U64 => self.run::<u64>(&device, config.into(), stream),
            SampleFormat::F32 => self.run::<f32>(&device, config.into(), stream),
            SampleFormat::F64 => self.run::<f64>(&device, config.into(), stream),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
        .unwrap();
    }

    pub fn run<T>(
        &mut self,
        device: &Device,
        config: StreamConfig,
        mut stream: Stream,
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
                // debug!("Callback");
                stream.fetch_data();
                for frame in data.chunks_mut(channels) {
                    for sample in frame.iter_mut() {
                        let value: T =
                            T::from_sample(stream.data.pop_front().unwrap_or(0) as f32 / 32768.);
                        *sample = value;
                    }
                }

                // write_data(data, channels, &mut next_value)
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
