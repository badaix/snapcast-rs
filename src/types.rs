//! # Snapcast protocol types

use std::{fmt, ops};

#[derive(Debug, Clone, Copy)]
pub struct SampleFormat {
    pub rate: u32,
    pub bits: u16,
    pub channels: u16,
}

impl fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.rate, self.bits, self.channels)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TimeStamp {
    pub sec: i32,
    pub usec: i32,
}

impl TimeStamp {
    pub fn new(sec: i32, usec: i32) -> Self {
        TimeStamp { sec, usec }
    }

    pub fn now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        TimeStamp {
            sec: now.as_secs() as i32,
            usec: now.subsec_micros() as i32,
        }
    }

    pub fn deserialize(buffer: &[u8]) -> Self {
        let sec = i32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let usec = i32::from_le_bytes(buffer[4..8].try_into().unwrap());
        TimeStamp { sec, usec }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&self.sec.to_le_bytes());
        buffer.extend_from_slice(&self.usec.to_le_bytes());
        buffer
    }
}

impl ops::Sub<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn sub(self, other: TimeStamp) -> TimeStamp {
        let mut sec = self.sec - other.sec;
        let mut usec = self.usec - other.usec;
        if usec < 0 {
            sec -= 1;
            usec += 1_000_000;
        }
        TimeStamp { sec, usec }
    }
}

impl ops::Add<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn add(self, other: TimeStamp) -> TimeStamp {
        let mut sec = self.sec + other.sec;
        let mut usec = self.usec + other.usec;
        if usec >= 1_000_000 {
            sec += 1;
            usec -= 1_000_000;
        }
        TimeStamp { sec, usec }
    }
}
