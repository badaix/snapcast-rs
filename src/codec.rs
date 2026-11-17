//! # Snapcast codec

use crate::messages::{CodecHeader, WireChunk};
use crate::types::{SampleFormat, TimeStamp};
use anyhow::{Result, bail};
use log::warn;
use std::fmt;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_FLAC, CodecParameters, DecoderOptions};
use symphonia::core::formats::Packet;

#[derive(Debug)]
pub struct PcmChunk {
    pub timestamp: TimeStamp,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct Wav {
    pub sample_format: SampleFormat,
}

#[derive(Debug)]
pub struct Null;

pub struct Flac {
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    pub sample_format: SampleFormat,
    packet_id: u64,
}

pub trait Codec {
    fn decode(&mut self, data: WireChunk) -> Option<PcmChunk>;
    fn get_sampleformat(&self) -> SampleFormat;
    fn get_codec_name(&self) -> &str;
}

impl fmt::Display for dyn Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "codec: {}, sample format: {}",
            self.get_codec_name(),
            self.get_sampleformat()
        )
    }
}

impl Codec for Wav {
    fn decode(&mut self, data: WireChunk) -> Option<PcmChunk> {
        Some(PcmChunk {
            timestamp: data.timestamp,
            payload: data.payload,
        })
    }

    fn get_sampleformat(&self) -> SampleFormat {
        self.sample_format
    }

    fn get_codec_name(&self) -> &str {
        "wav"
    }
}

impl Wav {
    pub fn new(sf: SampleFormat) -> Wav {
        Wav { sample_format: sf }
    }

    pub fn from_header(header: CodecHeader) -> Option<Wav> {
        // https://en.wikipedia.org/wiki/WAV
        // [Master RIFF chunk]
        //     FileTypeBlocID  (4 bytes) : Identifier « RIFF »  (0x52, 0x49, 0x46, 0x46)
        //     FileSize        (4 bytes) : Overall file size minus 8 bytes
        //     FileFormatID    (4 bytes) : Format = « WAVE »  (0x57, 0x41, 0x56, 0x45)
        //
        // [Chunk describing the data format]
        //     FormatBlocID    (4 bytes) : Identifier « fmt␣ »  (0x66, 0x6D, 0x74, 0x20)
        //     BlocSize        (4 bytes) : Chunk size minus 8 bytes, which is 16 bytes here  (0x10)
        //     AudioFormat     (2 bytes) : Audio format (1: PCM integer, 3: IEEE 754 float)
        //     NbrChannels     (2 bytes) : Number of channels
        //     Frequency       (4 bytes) : Sample rate (in hertz)
        //     BytePerSec      (4 bytes) : Number of bytes to read per second (Frequency * BytePerBloc).
        //     BytePerBloc     (2 bytes) : Number of bytes per block (NbrChannels * BitsPerSample / 8).
        //     BitsPerSample   (2 bytes) : Number of bits per sample
        if header.payload.len() < 36 {
            panic!(
                "WAV header too small ({} bytes, need >= 36)",
                header.payload.len()
            );
        }

        if &header.payload[0..4] != b"RIFF" {
            panic!("Not a WAV header (missing 'RIFF' identifier)");
        }
        if &header.payload[8..12] != b"WAVE" {
            panic!("Not a WAV header (missing 'WAVE' format)");
        }
        if &header.payload[12..16] != b"fmt " {
            panic!("WAVE audio format must be PCM (=1)");
        }
        let audio_format = u16::from_le_bytes(header.payload[20..22].try_into().unwrap());
        if audio_format != 1 {
            panic!("Not a WAV header (missing 'fmt ' identifier)");
        }

        let rate = u32::from_le_bytes(header.payload[24..28].try_into().unwrap());
        let bits = u16::from_le_bytes(header.payload[34..36].try_into().unwrap());
        let channels = u16::from_le_bytes(header.payload[22..24].try_into().unwrap());
        Some(Wav {
            sample_format: SampleFormat {
                rate,
                bits,
                channels,
            },
        })
    }
}

impl Null {
    pub fn new() -> Null {
        Null
    }
}

impl Default for Null {
    fn default() -> Self {
        Self::new()
    }
}

impl Codec for Null {
    fn decode(&mut self, _data: WireChunk) -> Option<PcmChunk> {
        None
    }

    // actually not really needed...
    fn get_sampleformat(&self) -> SampleFormat {
        SampleFormat {
            rate: 48000,
            bits: 16,
            channels: 2,
        }
    }

    fn get_codec_name(&self) -> &str {
        "null"
    }
}

impl Codec for Flac {
    fn decode(&mut self, data: WireChunk) -> Option<PcmChunk> {
        if data.payload.is_empty() {
            return None;
        }

        let packet = Packet::new_from_slice(0, self.packet_id, 0, &data.payload);
        self.packet_id += 1;

        let decoded = match self.decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(e) => {
                warn!("Failed to decode flac chunk, error: {}", e);
                return None;
            }
        };

        let spec = *decoded.spec();
        let frames = decoded.frames() as u64;

        let mut sample_buf = SampleBuffer::<i16>::new(frames, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();

        let mut out = Vec::with_capacity(samples.len() * 4);
        for &s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }

        Some(PcmChunk {
            timestamp: data.timestamp,
            payload: out,
        })
    }

    fn get_sampleformat(&self) -> SampleFormat {
        self.sample_format
    }

    fn get_codec_name(&self) -> &str {
        "flac"
    }
}

/// Parse FLAC STREAMINFO from a codec header payload.
///
/// Layout: "fLaC" (4) + block_header (4) + STREAMINFO (34)
/// STREAMINFO bytes 10-13 contain: sample_rate (20 bits) | channels-1 (3 bits) | bps-1 (5 bits)
fn parse_streaminfo(payload: &[u8]) -> Result<(SampleFormat, CodecParameters)> {
    if payload.len() < 42 {
        bail!(
            "FLAC header too small ({} bytes, need >= 42)",
            payload.len()
        );
    }
    if &payload[0..4] != b"fLaC" {
        bail!("not a FLAC header (missing fLaC magic)");
    }

    // STREAMINFO starts at offset 8 (after 4-byte magic + 4-byte block header)
    let si = &payload[8..];

    // Bytes 10-13 of STREAMINFO (offsets 10..14 from si start):
    // 20 bits sample rate | 3 bits (channels-1) | 5 bits (bps-1) | 36 bits total samples
    let b10 = si[10] as u32;
    let b11 = si[11] as u32;
    let b12 = si[12] as u32;

    let sample_rate = (b10 << 12) | (b11 << 4) | (b12 >> 4);
    let channels = ((b12 >> 1) & 0x07) + 1;
    let bits_per_sample = (((b12 & 0x01) << 4) | (si[13] as u32 >> 4)) + 1;

    let sf = SampleFormat {
        rate: sample_rate,
        bits: bits_per_sample as u16,
        channels: channels as u16,
    };

    let mut params = CodecParameters::new();
    params
        .for_codec(CODEC_TYPE_FLAC)
        .with_sample_rate(sample_rate)
        .with_bits_per_sample(bits_per_sample)
        .with_channels(
            symphonia::core::audio::Channels::from_bits(((1u64 << channels) - 1) as u32)
                .unwrap_or(symphonia::core::audio::Channels::FRONT_LEFT),
        )
        .with_extra_data(payload[8..42].to_vec().into_boxed_slice());

    Ok((sf, params))
}

impl Flac {
    pub fn from_header(header: CodecHeader) -> Option<Flac> {
        let (sf, params) = parse_streaminfo(&header.payload).unwrap();
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| anyhow::anyhow!("failed to create FLAC decoder: {e}"))
            .unwrap();

        Some(Self {
            decoder,
            sample_format: sf,
            packet_id: 0,
        })
    }
}
