//! Diff time with the server

use crate::types::TimeStamp;
// use log::debug;
use std::collections::VecDeque;

type TimeDiff = TimeStamp;

pub struct TimeSync {
    diffs: VecDeque<TimeDiff>,
}

impl TimeSync {
    pub fn new() -> Self {
        TimeSync {
            diffs: VecDeque::new(),
        }
    }

    pub fn add(&mut self, diff: TimeDiff) {
        self.diffs.push_back(diff);
        while self.diffs.len() > 100 {
            self.diffs.pop_front();
        }
    }
}

impl Default for TimeSync {
    fn default() -> Self {
        Self::new()
    }
}
