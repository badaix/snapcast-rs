//! Diff time with the server

use log::debug;

use crate::types::TimeStamp;
// use log::debug;
use std::collections::VecDeque;

type TimeDiff = TimeStamp;

pub struct TimeSync {
    diffs: VecDeque<TimeDiff>,
    diff: TimeDiff,
}

impl TimeSync {
    pub fn new() -> Self {
        TimeSync {
            diffs: VecDeque::new(),
            diff: TimeDiff::new(0, 0),
        }
    }

    pub fn add(&mut self, c2s: &TimeDiff, s2c: &TimeDiff) {
        let mut diff = *c2s - *s2c;
        debug!("c2s: {:?}, s2c: {:?}, diff: {:?}", c2s, s2c, diff);
        let odd: bool = diff.sec % 2 == 1;
        diff.sec /= 2;
        diff.usec /= 2;
        if odd {
            diff.usec += 500000;
        }

        // let diff = (c2s.sec as f64 / 2. - s2c.sec as f64 / 2.) * 1000.
        //     + (c2s.usec as f64 / 2. - s2c.usec as f64 / 2.) / 1000.;

        self.diffs.push_back(diff);
        while self.diffs.len() > 100 {
            self.diffs.pop_front();
        }
        let mut v = Vec::from(self.diffs.clone());
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        self.diff = v[v.len() / 2];
        debug!(
            "c2s: {:?}, s2c: {:?}, diff: {:?}, median: {:?}",
            c2s, s2c, diff, self.diff
        );
    }

    pub fn get_diff(&self) -> TimeDiff {
        self.diff
    }
}

impl Default for TimeSync {
    fn default() -> Self {
        Self::new()
    }
}
