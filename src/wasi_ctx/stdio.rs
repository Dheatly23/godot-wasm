use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::Result as IoResult;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::Instant;

use godot::prelude::*;
use wasi_isolated_fs::stdio::{HostStdin, HostStdout};

use crate::godot_util::SendSyncWrapper;

pub struct PackedByteArrayReader {
    data: SendSyncWrapper<PackedByteArray>,
    cursor: AtomicUsize,
}

impl Debug for PackedByteArrayReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("PackedByteArrayReader")
            .field("data_len", &self.data.len())
            .field("cursor", &self.cursor.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl From<PackedByteArray> for PackedByteArrayReader {
    fn from(v: PackedByteArray) -> Self {
        Self {
            data: SendSyncWrapper::new(v),
            cursor: AtomicUsize::new(0),
        }
    }
}

impl PackedByteArrayReader {
    fn advance(&self, l: usize) -> Range<usize> {
        let e = self.data.len();
        let mut c = self.cursor.load(Ordering::Acquire);
        loop {
            let i = c.saturating_add(l).min(e);
            let Err(v) =
                self.cursor
                    .compare_exchange_weak(c, i, Ordering::AcqRel, Ordering::Acquire)
            else {
                return c..i;
            };
            c = v;
        }
    }

    fn maybe_sleep(t: Option<Instant>) {
        if let Some(t) = t
            .and_then(|t| t.checked_duration_since(Instant::now()))
            .filter(|t| !t.is_zero())
        {
            sleep(t);
        }
    }
}

impl HostStdin for PackedByteArrayReader {
    fn read(&self, len: usize) -> IoResult<Vec<u8>> {
        let r = self.advance(len);
        Ok(self.data.as_slice()[r].to_owned())
    }

    fn read_block(&self, len: usize, timeout: Option<Instant>) -> IoResult<Vec<u8>> {
        let r = self.advance(len);
        let r = self.data.as_slice()[r].to_owned();
        if r.is_empty() {
            Self::maybe_sleep(timeout);
        }
        Ok(r)
    }

    fn skip(&self, len: usize) -> IoResult<usize> {
        Ok(self.advance(len).len())
    }

    fn skip_block(&self, len: usize, timeout: Option<Instant>) -> IoResult<usize> {
        let r = self.advance(len);
        if r.is_empty() {
            Self::maybe_sleep(timeout);
        }
        Ok(r.len())
    }

    fn block(&self, timeout: Option<Instant>) -> IoResult<()> {
        if self.cursor.load(Ordering::Acquire) >= self.data.len() {
            Self::maybe_sleep(timeout);
        }
        Ok(())
    }
}

pub struct StdoutCbUnbuffered<F>(F);

impl<F> Debug for StdoutCbUnbuffered<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("StdoutCbUnbuffered").finish_non_exhaustive()
    }
}

impl<F> StdoutCbUnbuffered<F>
where
    F: Fn(&[u8]),
{
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

impl<F> HostStdout for StdoutCbUnbuffered<F>
where
    F: Fn(&[u8]),
{
    fn write(&self, buf: &[u8]) -> IoResult<()> {
        let f = &self.0;
        f(buf);
        Ok(())
    }

    fn flush(&self) -> IoResult<()> {
        Ok(())
    }
}
