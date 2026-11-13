//! # Snapcast codec

use crate::messages::{CodecHeader, SampleFormat, TimeStamp, WireChunk};

#[derive(Debug)]
pub struct PcmChunk {
    pub timestamp: TimeStamp,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct Wav {
    pub sample_format: SampleFormat,
}

pub trait Codec {
    // fn from_header(header: CodecHeader) -> Option<Self> where Self: Sized;
    fn decode(&self, data: WireChunk) -> PcmChunk;
}

impl Codec for Wav {
    fn decode(&self, data: WireChunk) -> PcmChunk {
        PcmChunk {
            timestamp: data.timestamp,
            payload: data.payload,
        }
    }
}

impl Wav {
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
