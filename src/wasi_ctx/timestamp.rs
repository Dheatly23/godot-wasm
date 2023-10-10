use std::mem::transmute;
use std::sync::atomic::*;
use std::time::{Duration, SystemTime};

pub fn to_unix_time(time: SystemTime) -> i128 {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i128::from(d.as_secs()),
        Err(d) => {
            let d = d.duration();
            let mut r = -i128::from(d.as_secs());
            if d.subsec_nanos() > 0 {
                r = r.saturating_sub(1);
            }
            r
        }
    }
}

pub fn from_unix_time(time: i64) -> Option<SystemTime> {
    if time >= 0 {
        // SAFETY: Reinterprets i64 as u64
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(unsafe { transmute(time) }))
    } else {
        // SAFETY: Reinterprets i64 as u64
        SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(unsafe {
            transmute(time.wrapping_neg())
        }))
    }
}

cfg_if::cfg_if! {
    if #[cfg(target_has_atomic = "64")] {
        type AtomicType = AtomicI64;
        type ValueType = i64;
    } else if #[cfg(target_has_atomic = "32")] {
        type AtomicType = AtomicI32;
        type ValueType = i32;
    } else if #[cfg(target_has_atomic = "16")] {
        type AtomicType = AtomicI16;
        type ValueType = i16;
    } else if #[cfg(target_has_atomic = "8")] {
        type AtomicType = AtomicI8;
        type ValueType = i8;
    } else {
        compile_error!("No atomics available!");
    }
}

#[derive(Debug)]
pub struct Timestamp(AtomicType);

impl Timestamp {
    fn from_systemtime(time: SystemTime) -> ValueType {
        let v = to_unix_time(time);
        match ValueType::try_from(v) {
            Ok(v) => v,
            Err(_) if v >= 0 => ValueType::MAX,
            Err(_) => ValueType::MIN,
        }
    }

    pub fn new(time: SystemTime) -> Self {
        Self(AtomicType::new(Self::from_systemtime(time)))
    }

    pub fn get_stamp(&self) -> Option<SystemTime> {
        // CLIPPY: Allow this for 64-bit system
        #[allow(clippy::unnecessary_cast)]
        let s = self.0.load(Ordering::Acquire) as i64;

        from_unix_time(s)
    }

    pub fn set_stamp(&self, time: SystemTime) {
        self.0.store(Self::from_systemtime(time), Ordering::Release);
    }
}

pub struct FileTimestamp {
    pub ctime: SystemTime,
    pub mtime: Timestamp,
    pub atime: Timestamp,
}

impl FileTimestamp {
    #[inline]
    pub fn new() -> Self {
        Self::with_time(SystemTime::now())
    }

    #[inline]
    pub fn with_time(time: SystemTime) -> Self {
        Self {
            ctime: time,
            mtime: Timestamp::new(time),
            atime: Timestamp::new(time),
        }
    }
}
