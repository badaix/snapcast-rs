//! # Snapcast protocol types

use std::fmt;

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

#[derive(Debug)]
pub struct TimeStamp {
    pub sec: i32,
    pub usec: i32,
}
