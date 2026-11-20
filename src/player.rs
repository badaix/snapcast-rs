//! Audio player based on cpal

use std::sync::atomic::{AtomicI16, Ordering::Relaxed};

use cpal::{
    Device, Error, ErrorKind, FromSample, OutputCallbackInfo, SizedSample, StreamConfig,
    SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
// use log::debug;
use crate::{stream::AudioStream, timesync::TimeDiff};
use std::sync::{Arc, Mutex};

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
            move |data: &mut [T], info: &OutputCallbackInfo| {
                let when = info.timestamp().playback - info.timestamp().callback;

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
                let samples = audiostream.get_player_cbunk(
                    &TimeDiff::from_duration(when),
                    data.chunks(channels).len() as u32,
                );
                let mut sample_idx = 0;
                for frame in data.chunks_mut(channels) {
                    for sample in frame.iter_mut() {
                        *sample = T::from_sample(samples[sample_idx] * vol);
                        sample_idx += 1;
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
