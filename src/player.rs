//! Audio player based on cpal

use cpal::{
    Device, Error, ErrorKind, FromSample, I24, OutputCallbackInfo, Sample, SampleFormat,
    SizedSample, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use log::debug;

pub struct Player {
    stream: Option<cpal::Stream>,
}

impl Player {
    pub fn new() -> Self {
        Player { stream: None }
    }

    pub fn play(&mut self) {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");

        let config = device.default_output_config().unwrap();
        println!("Default output config: {config:?}");

        match config.sample_format() {
            SampleFormat::I8 => self.run::<i8>(&device, config.into()),
            SampleFormat::I16 => self.run::<i16>(&device, config.into()),
            SampleFormat::I24 => self.run::<I24>(&device, config.into()),
            SampleFormat::I32 => self.run::<i32>(&device, config.into()),
            // SampleFormat::I48 => self.run::<I48>(&device, config.into()),
            SampleFormat::I64 => self.run::<i64>(&device, config.into()),
            SampleFormat::U8 => self.run::<u8>(&device, config.into()),
            SampleFormat::U16 => self.run::<u16>(&device, config.into()),
            // SampleFormat::U24 => self.run::<U24>(&device, config.into()),
            SampleFormat::U32 => self.run::<u32>(&device, config.into()),
            // SampleFormat::U48 => self.run::<U48>(&device, config.into()),
            SampleFormat::U64 => self.run::<u64>(&device, config.into()),
            SampleFormat::F32 => self.run::<f32>(&device, config.into()),
            SampleFormat::F64 => self.run::<f64>(&device, config.into()),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
        .unwrap();
    }

    pub fn run<T>(&mut self, device: &Device, config: StreamConfig) -> Result<(), anyhow::Error>
    where
        T: SizedSample + FromSample<f32>,
    {
        let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;

        // Produce a sinusoid of maximum amplitude.
        let mut sample_clock = 0f32;
        let mut next_value = move || {
            sample_clock = (sample_clock + 1.0) % sample_rate;
            (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
        };

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
                write_data(data, channels, &mut next_value)
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

// fn write_silence<T>(data: &mut [T], _: &cpal::OutputCallbackInfo)
// where
//     T: Sample + FromSample<f32>,
// {
//     debug!("write silence");
//     for sample in data.iter_mut() {
//         *sample = T::from_sample(rand::random_range(-10000..10000) as f32 / 100.0); // Sample::EQUILIBRIUM;
//     }
// }

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    debug!("write data");
    for frame in output.chunks_mut(channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
