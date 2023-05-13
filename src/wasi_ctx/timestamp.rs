use std::mem::transmute;
use std::sync::atomic::*;
use std::time::{Duration, SystemTime};

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
        match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => match ValueType::try_from(d.as_secs()) {
                Ok(v) => v,
                Err(_) => ValueType::MAX,
            },
            Err(e) => {
                let d = e.duration();
                let mut v = d.as_secs();
                if d.subsec_nanos() > 0 {
                    v = v.saturating_add(1);
                }
                v = (!v).wrapping_add(1);

                // SAFETY: Interconvert between u/i64 to prevent panics on debug build.
                let v_: i64 = unsafe { transmute(v) };
                match ValueType::try_from(v_) {
                    Ok(v) if v_ <= 0 => v,
                    _ => ValueType::MIN,
                }
            }
        }
    }

    pub fn new(time: SystemTime) -> Self {
        Self(AtomicType::new(Self::from_systemtime(time)))
    }

    pub fn get_stamp(&self) -> Option<SystemTime> {
        let s = self.0.load(Ordering::Acquire) as i64;

        // SAFETY: Interconvert between u/i64 to prevent panics on debug build.
        let s_: u64 = unsafe { transmute(s) };
        if s >= 0 {
            SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(s_))
        } else {
            SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs((!s_).wrapping_add(1)))
        }
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
