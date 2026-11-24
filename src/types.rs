//! # Snapcast protocol types

use std::{cmp::Ordering, fmt, ops};

use std::sync::LazyLock;
use std::time::{Duration, Instant};

static START: LazyLock<Instant> = LazyLock::new(Instant::now);

#[derive(Debug, Clone, Copy)]
pub struct SampleFormat {
    pub rate: u32,
    pub bits: u16,
    pub channels: u16,
}

impl SampleFormat {
    pub fn get_framesize(&self) -> u16 {
        let mut bits = self.bits;
        if bits == 24 {
            bits = 32;
        }

        bits / 8 * self.channels
    }
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

    pub fn from_duration(duration: Duration) -> Self {
        TimeStamp {
            sec: duration.as_secs() as i64,
            usec: duration.subsec_micros() as i32,
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

    pub fn to_duration(&self) -> Option<Duration> {
        if self.sec < 0 {
            None
        } else {
            Some(Duration::from_micros(
                self.sec as u64 * 1_000_000_u64 + self.usec as u64,
            ))
        }
    }

    pub fn as_micros(&self) -> i64 {
        self.sec * 1_000_000 + self.usec as i64
    }

    pub fn as_millis(&self) -> i64 {
        self.sec * 1_000 + self.usec as i64 / 1_000_i64
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

impl ops::Add<Duration> for TimeStamp {
    type Output = TimeStamp;

    fn add(self, other: Duration) -> TimeStamp {
        let ts = TimeStamp::from_duration(other);
        self + ts
    }
}

impl ops::Sub<Duration> for TimeStamp {
    type Output = TimeStamp;

    fn sub(self, other: Duration) -> TimeStamp {
        let ts = TimeStamp::from_duration(other);
        self - ts
    }
}

impl fmt::Display for TimeStamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:06}s", self.sec, self.usec)
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

    #[test]
    fn test_timestamp_plus_duration() {
        let t = TimeStamp::new(5, 500_000);
        let d = Duration::from_millis(3800);
        let result = t + d;
        assert_eq!(result.sec, 9);
        assert_eq!(result.usec, 300_000);
    }

    #[test]
    fn test_timestamp_minus_duration() {
        let t = TimeStamp::new(5, 500_000);
        let d = Duration::from_millis(3800);
        let result = t - d;
        assert_eq!(result.sec, 1);
        assert_eq!(result.usec, 700_000);
    }
}
