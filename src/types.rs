//! # Snapcast protocol types

use std::{cmp::Ordering, fmt, ops};

use std::sync::LazyLock;
use std::time::Instant;

static START: LazyLock<Instant> = LazyLock::new(Instant::now);

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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TimeStamp {
    pub sec: i64,
    pub usec: i32,
}

impl TimeStamp {
    pub fn new(sec: i64, usec: i32) -> Self {
        TimeStamp { sec, usec }
    }

    pub fn now() -> Self {
        let now = START.elapsed();
        TimeStamp {
            sec: now.as_secs() as i64,
            usec: now.subsec_micros() as i32,
        }
    }

    pub fn deserialize(buffer: &[u8]) -> Self {
        let sec = i32::from_le_bytes(buffer[0..4].try_into().unwrap()) as i64;
        let usec = i32::from_le_bytes(buffer[4..8].try_into().unwrap());
        TimeStamp { sec, usec }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&(self.sec as i32).to_le_bytes());
        buffer.extend_from_slice(&self.usec.to_le_bytes());
        buffer
    }
}

impl PartialOrd for TimeStamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.sec != other.sec {
            Some(self.sec.cmp(&other.sec))
        } else {
            Some(self.usec.cmp(&other.usec))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_subtraction() {
        let t1 = TimeStamp::new(5, 500_000);
        let t2 = TimeStamp::new(3, 800_000);
        let result = t1 - t2;
        assert_eq!(result.sec, 1);
        assert_eq!(result.usec, 700_000);
    }

    #[test]
    fn test_timestamp_addition() {
        let t1 = TimeStamp::new(5, 500_000);
        let t2 = TimeStamp::new(3, 800_000);
        let result = t1 + t2;
        assert_eq!(result.sec, 9);
        assert_eq!(result.usec, 300_000);
    }

    #[test]
    fn test_timestamp_compare() {
        let t1 = TimeStamp::new(5, 500_000);
        let t2 = TimeStamp::new(3, 800_000);
        assert!(t1 > t2);
        assert!(t2 < t1);

        let t1 = TimeStamp::new(3, 800_000);
        let t2 = TimeStamp::new(3, 800_000);
        assert!(t1 == t2);

        let t1 = TimeStamp::new(5, 500_000);
        let t2 = TimeStamp::new(5, 600_000);
        assert!(t1 < t2);

        let t1 = TimeStamp::new(1, 100_000);
        let t2 = TimeStamp::new(1, 200_000);
        assert!(t1 < t2);
    }
}
