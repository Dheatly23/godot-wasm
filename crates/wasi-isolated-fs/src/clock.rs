use std::io::{Error as IoError, ErrorKind};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::Result as AnyResult;

use crate::bindings::wasi;
use crate::errors;

const MAX_TIMEOUT: Duration = Duration::from_secs(1);

pub struct ClockController {
    epoch: Instant,
}

impl Default for ClockController {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockController {
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }

    pub fn now(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as _
    }

    pub fn poll_for(&self, dur: u64) -> AnyResult<ClockPollable> {
        match Instant::now().checked_add(Duration::from_nanos(dur)) {
            Some(until) => Ok(ClockPollable { until }),
            None => Err(errors::MonotonicClockError.into()),
        }
    }

    pub fn poll_until(&self, stamp: u64) -> AnyResult<ClockPollable> {
        match self.epoch.checked_add(Duration::from_nanos(stamp)) {
            Some(until) => Ok(ClockPollable { until }),
            None => Err(errors::MonotonicClockError.into()),
        }
    }
}

pub struct ClockPollable {
    pub(crate) until: Instant,
}

impl ClockPollable {
    pub fn is_ready(&self) -> bool {
        Instant::now() >= self.until
    }

    pub fn block(&self) -> AnyResult<()> {
        let stamp = Instant::now();
        loop {
            let d = self.until.saturating_duration_since(Instant::now());
            if d.is_zero() {
                return Ok(());
            }

            sleep(d.min(MAX_TIMEOUT));
            if stamp.elapsed() >= MAX_TIMEOUT {
                return Err(IoError::from(ErrorKind::TimedOut).into());
            }
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UTCClock;

impl wasi::clocks::timezone::Host for UTCClock {
    fn display(
        &mut self,
        _: wasi::clocks::timezone::Datetime,
    ) -> AnyResult<wasi::clocks::timezone::TimezoneDisplay> {
        Ok(wasi::clocks::timezone::TimezoneDisplay {
            utc_offset: 0,
            in_daylight_saving_time: false,
            name: "UTC".into(),
        })
    }

    fn utc_offset(&mut self, _: wasi::clocks::timezone::Datetime) -> AnyResult<i32> {
        Ok(0)
    }
}
