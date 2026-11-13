//! # Snapcast protocol types

#[derive(Debug)]
pub struct SampleFormat {
    pub rate: u32,
    pub bits: u16,
    pub channels: u16,
}

#[derive(Debug)]
pub struct TimeStamp {
    pub sec: i32,
    pub usec: i32,
}
