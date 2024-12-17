use std::io::{
    stderr, stdout, Error as IoError, ErrorKind, IoSlice, Result as IoResult, Stderr, Stdout, Write,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result as AnyResult;
use memchr::memchr;
use parking_lot::{Condvar, Mutex};
use scopeguard::{defer, guard, guard_on_unwind};
use smallvec::SmallVec;

const MAX_TIMEOUT: Duration = Duration::from_secs(1);

pub struct StdinSignal {
    inner: Mutex<StdinInner>,
    cond: Condvar,
    f: Box<dyn Fn() + Send + Sync>,
}

pub struct StdinProvider(Arc<StdinSignal>);

type StdinInnerData = SmallVec<[u8; 32]>;

struct StdinInner {
    closed: bool,
    data: StdinInnerData,
    start: usize,
    end: usize,
}

impl StdinInner {
    fn len(&self) -> usize {
        (self.end as isize - self.start as isize).wrapping_rem_euclid(self.data.len() as isize)
            as usize
    }

    fn push_data(&mut self, buf: &[u8]) {
        let old_size = self.len();

        while self
            .len()
            .checked_add(buf.len())
            .filter(|&v| v < self.data.len())
            .is_none()
        {
            // Resize
            let old_len = self.data.len();
            let new_len = old_len.checked_mul(2).unwrap();
            self.data.reserve_exact(old_len);
            self.data.resize(new_len, 0);
            if self.end < self.start {
                self.data.copy_within(..self.end, old_len);
                self.end += old_len;
            }
        }

        let i = self.end + buf.len();
        if i > self.data.len() {
            let (a, b) = buf.split_at(self.data.len() - self.end);
            self.data[self.end..].copy_from_slice(a);
            self.data[..b.len()].copy_from_slice(b);
            self.end = b.len();
        } else {
            self.data[self.end..i].copy_from_slice(buf);
            self.end = i % self.data.len();
        }

        debug_assert_eq!(
            self.len(),
            old_size + buf.len(),
            "invalid state of ringbuffer (length: {}, start: {}, end: {}, input length: {})",
            self.data.len(),
            self.start,
            self.end,
            buf.len()
        );
    }

    fn pop_data(&mut self, len: usize) -> (&[u8], &[u8]) {
        let old_size = self.len();

        let ret: (&[u8], &[u8]) = if self.end >= self.start {
            let i = self.start + len.min(self.end);
            let s = &self.data[self.start..i];
            self.start = i;
            (s, &[])
        } else if let Some(i) = len.checked_add(self.start).filter(|&v| v < self.data.len()) {
            let s = &self.data[self.start..i];
            self.start = i;
            (s, &[])
        } else {
            let i = (len - (self.data.len() - self.start)).min(self.end);
            let (a, b) = (&self.data[self.start..], &self.data[..i]);
            self.start = i;
            (a, b)
        };

        debug_assert_eq!(
            self.len(),
            old_size.saturating_sub(len),
            "invalid state of ringbuffer (length: {}, start: {}, end: {}, output length: {})",
            self.data.len(),
            self.start,
            self.end,
            len
        );
        ret
    }
}

impl StdinSignal {
    pub fn new(f: Box<dyn Fn() + Send + Sync>) -> (Arc<Self>, StdinProvider) {
        let ret = Arc::new(Self {
            inner: Mutex::new(StdinInner {
                closed: false,
                data: StdinInnerData::from_buf(Default::default()),
                start: 0,
                end: 0,
            }),
            cond: Condvar::new(),
            f,
        });

        (ret.clone(), StdinProvider(ret))
    }

    pub fn is_ready(&self) -> bool {
        let guard = self.inner.lock();
        guard.closed || guard.end != guard.start
    }

    pub fn read(&self, len: usize) -> IoResult<Vec<u8>> {
        let mut guard = self.inner.lock();

        let (a, b) = guard.pop_data(len);

        let mut ret = vec![0u8; a.len() + b.len()];
        ret[..a.len()].copy_from_slice(a);
        ret[a.len()..].copy_from_slice(b);
        Ok(ret)
    }

    pub fn read_block(&self, len: usize) -> IoResult<Vec<u8>> {
        let timeout = Instant::now() + MAX_TIMEOUT;
        let mut guard = self.inner.lock();

        let (a, b) = loop {
            let closed = guard.closed;
            let (a, b) = guard.pop_data(len);
            if !closed && a.is_empty() && b.is_empty() {
                (self.f)();
                if self.cond.wait_until(&mut guard, timeout).timed_out() {
                    return Err(ErrorKind::TimedOut.into());
                }
            } else {
                break (a, b);
            }
        };

        let mut ret = vec![0u8; a.len() + b.len()];
        ret[..a.len()].copy_from_slice(a);
        ret[a.len()..].copy_from_slice(b);
        Ok(ret)
    }

    pub fn skip(&self, len: usize) -> IoResult<usize> {
        let mut guard = self.inner.lock();

        let (a, b) = guard.pop_data(len);
        Ok(a.len() + b.len())
    }

    pub fn skip_block(&self, len: usize) -> IoResult<usize> {
        let timeout = Instant::now() + MAX_TIMEOUT;
        let mut guard = self.inner.lock();

        let (a, b) = loop {
            let closed = guard.closed;
            let (a, b) = guard.pop_data(len);
            if !closed && a.is_empty() && b.is_empty() {
                (self.f)();
                if self.cond.wait_until(&mut guard, timeout).timed_out() {
                    return Err(ErrorKind::TimedOut.into());
                }
            } else {
                break (a, b);
            }
        };

        Ok(a.len() + b.len())
    }

    pub fn poll(self: &Arc<Self>) -> AnyResult<StdinSignalPollable> {
        Ok(StdinSignalPollable(self.clone()))
    }
}

impl StdinProvider {
    pub fn consumer(&self) -> Arc<StdinSignal> {
        self.0.clone()
    }

    pub fn is_paired(&self, other: &Arc<StdinSignal>) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    pub fn write(&self, buf: &[u8]) {
        let mut guard = self.0.inner.lock();
        if guard.closed {
            return;
        }
        guard.push_data(buf);
        self.0.cond.notify_one();
    }

    pub fn close(&self) {
        let mut guard = self.0.inner.lock();
        guard.closed = true;
        self.0.cond.notify_one();
    }
}

pub struct StdinSignalPollable(Arc<StdinSignal>);

impl StdinSignalPollable {
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        self.0.is_ready()
    }

    pub fn block(&self) -> AnyResult<()> {
        let timeout = Instant::now() + MAX_TIMEOUT;
        let mut guard = self.0.inner.lock();
        while !guard.closed && guard.end == guard.start {
            if self.0.cond.wait_until(&mut guard, timeout).timed_out() {
                return Err(IoError::from(ErrorKind::TimedOut).into());
            }
        }

        Ok(())
    }
}

pub struct LineBuffer {
    buf: Box<[u8; 1024]>,
    len: usize,
    s: String,
}

const LINESEP: u8 = b'\n';
const REPLACEMENT: &str = "\u{FFFD}";

impl Default for LineBuffer {
    fn default() -> Self {
        Self {
            buf: Box::new([0; 1024]),
            len: 0,
            s: String::new(),
        }
    }
}

impl LineBuffer {
    pub fn write<F, E>(&mut self, mut f: F, mut data: &[u8]) -> Result<(), E>
    where
        for<'a> F: FnMut(&'a str) -> Result<(), E>,
    {
        let Self { buf, len, s } = &mut *guard_on_unwind(self, |this| this.len = 0);

        while !data.is_empty() {
            let mut d;
            (d, data) = if let Some(i) = memchr(LINESEP, data) {
                data.split_at(i + 1)
            } else {
                (data, &[] as &[_])
            };

            while !d.is_empty() {
                if *len == 0 && *d.last().unwrap() == LINESEP {
                    // Emit line without buffering
                    let mut it = d.utf8_chunks();
                    let mut chunk = it.next().unwrap();
                    if chunk.invalid().is_empty() {
                        f(chunk.valid())?;
                        d = &[];
                        continue;
                    }

                    s.clear();
                    s.reserve(d.len().min(buf.len()));
                    loop {
                        let mut v = chunk.valid();

                        // Limit string length
                        while s.len() + v.len() >= buf.len() {
                            if s.is_empty() {
                                // Buffer string is empty
                                f(v)?;
                                v = "";
                                continue;
                            }

                            // Split at floor char boundary
                            let i = (0..=buf.len().saturating_sub(s.len()).min(v.len()))
                                .rfind(|&i| v.is_char_boundary(i))
                                .unwrap_or(0);
                            let t;
                            (t, v) = v.split_at(i);
                            *s += t;

                            // Emit chunk
                            f(s)?;
                            s.clear();
                        }

                        *s += v;
                        if !chunk.invalid().is_empty() {
                            *s += REPLACEMENT;
                        }

                        chunk = match it.next() {
                            Some(v) => v,
                            None => break,
                        };
                    }

                    if !s.is_empty() {
                        f(s)?;
                    }
                    d = &[];
                    continue;
                }

                let rest = buf.len() - *len;
                if rest >= d.len() {
                    let e = *len + d.len();
                    buf[*len..e].copy_from_slice(d);
                    (*len, d) = (e, &[]);
                } else {
                    let a;
                    (a, d) = d.split_at(rest);
                    buf[*len..].copy_from_slice(a);
                    *len = buf.len();
                }

                if buf[*len - 1] == LINESEP {
                    // Emit the line
                    let mut it = buf[..*len].utf8_chunks();
                    let chunk = it.next().unwrap();
                    let len = guard(&mut *len, |len| *len = 0);
                    f(if chunk.invalid().is_empty() {
                        chunk.valid()
                    } else {
                        s.clear();
                        s.reserve(**len);
                        *s += chunk.valid();
                        *s += REPLACEMENT;
                        for chunk in it {
                            *s += chunk.valid();
                            if !chunk.invalid().is_empty() {
                                *s += REPLACEMENT;
                            }
                        }

                        s
                    })?;
                    continue;
                } else if *len < buf.len() {
                    continue;
                }

                // Buffer full
                let bp = &raw mut *buf;
                let mut it = buf.utf8_chunks();
                let chunk = it.next().unwrap();
                let i = chunk.invalid().len();
                if i == 0 {
                    // All of it is valid
                    defer! {
                        *len = 0;
                    }
                    f(chunk.valid())?;
                    continue;
                } else if chunk.valid().len() + i == buf.len() {
                    // Some invalid residual in buffer
                    defer! {
                        // SAFETY: Buffer is not used and alive
                        let buf = unsafe { &mut *bp };
                        let s = buf.len() - i;
                        buf.copy_within(s.., 0);
                        *len = i;
                    }
                    f(chunk.valid())?;
                    continue;
                }

                s.clear();
                s.reserve(buf.len());
                *s += chunk.valid();
                *s += REPLACEMENT;
                let mut chunk = it.next();
                while let Some(c) = chunk {
                    *s += c.valid();
                    let i = c.invalid().len();
                    chunk = it.next();

                    if chunk.is_none() {
                        if i > 0 {
                            // Some invalid residual in buffer
                            let s = buf.len() - i;
                            buf.copy_within(s.., 0);
                        }
                        *len = i;
                        break;
                    } else if i != 0 {
                        *s += REPLACEMENT;
                    }
                }

                debug_assert!(*len < buf.len(), "length is not set");
                f(s)?;
            }
        }

        Ok(())
    }

    pub fn flush<F, E>(&mut self, mut f: F) -> Result<(), E>
    where
        for<'a> F: FnMut(&'a str) -> Result<(), E>,
    {
        let mut g = guard(self, |this| this.len = 0);
        let Self { buf, len, s } = &mut *g;

        let mut it = buf[..*len].utf8_chunks();
        let Some(chunk) = it.next() else {
            return Ok(());
        };
        if chunk.invalid().is_empty() {
            *len = 0;
            return f(chunk.valid());
        }

        s.clear();
        s.reserve(*len);
        *s += chunk.valid();
        *s += REPLACEMENT;
        for chunk in it {
            *s += chunk.valid();
            if !chunk.invalid().is_empty() {
                *s += REPLACEMENT;
            }
        }

        *len = 0;
        f(s)
    }
}

type StdoutCb = Box<dyn Send + Sync + FnMut(&[u8])>;

pub struct StdoutCbLineBuffered {
    buf: LineBuffer,
    cb: StdoutCb,
}

impl StdoutCbLineBuffered {
    pub fn new(f: impl 'static + Send + Sync + FnMut(&[u8])) -> Self {
        Self {
            buf: Default::default(),
            cb: Box::new(f),
        }
    }

    fn split(&mut self) -> (&mut LineBuffer, impl use<'_> + FnMut(&str) -> IoResult<()>) {
        let Self { buf, cb } = self;
        (buf, |s| {
            cb(s.as_bytes());
            Ok(())
        })
    }

    pub fn poll(&self) -> AnyResult<NullPollable> {
        Ok(NullPollable::new())
    }
}

impl Write for StdoutCbLineBuffered {
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        let (lb, f) = self.split();
        lb.write(f, buf)
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        let (lb, f) = self.split();
        lb.flush(f)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        let mut ret = 0;
        for buf in bufs {
            self.write_all(buf)?;
            ret += buf.len();
        }
        Ok(ret)
    }
}

pub struct StdoutCbBlockBuffered {
    buf: Box<[u8; 1024]>,
    len: usize,
    cb: StdoutCb,
}

impl StdoutCbBlockBuffered {
    pub fn new(f: impl 'static + Send + Sync + FnMut(&[u8])) -> Self {
        Self {
            buf: Box::new([0; 1024]),
            len: 0,
            cb: Box::new(f),
        }
    }

    pub fn poll(&self) -> AnyResult<NullPollable> {
        Ok(NullPollable::new())
    }
}

impl Write for StdoutCbBlockBuffered {
    fn write_all(&mut self, mut buf: &[u8]) -> IoResult<()> {
        let cb = &mut self.cb;
        while !buf.is_empty() {
            if self.len == 0 && buf.len() >= self.buf.len() {
                let a;
                (a, buf) = buf.split_at(self.buf.len());
                cb(a);
                continue;
            }

            let i = self.buf.len() - self.len;
            if i >= buf.len() {
                let e = self.len + buf.len();
                self.buf[self.len..e].copy_from_slice(buf);
                (self.len, buf) = (e, &[]);
            } else {
                let a;
                (a, buf) = buf.split_at(i);
                self.buf[self.len..].copy_from_slice(a);
                self.len = self.buf.len();
            }

            debug_assert!(
                self.len <= self.buf.len(),
                "{} > {}",
                self.len,
                self.buf.len()
            );
            if self.len >= self.buf.len() {
                let len = &mut self.len;
                defer! {
                    *len = 0;
                }
                cb(&self.buf[..]);
            }
        }

        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        let cb = &mut self.cb;
        let len = guard(&mut self.len, |len| *len = 0);
        if **len > 0 {
            cb(&self.buf[..**len]);
        }
        Ok(())
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        let mut ret = 0;
        for buf in bufs {
            self.write_all(buf)?;
            ret += buf.len();
        }
        Ok(ret)
    }
}

struct StdBypassInner<T>(LineBuffer, T);

impl<T: Write> Write for StdBypassInner<T> {
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        let (lb, f) = self.split();
        lb.write(f, buf)
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        let (lb, f) = self.split();
        lb.flush(f)?;
        self.1.flush()
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        let mut ret = 0;
        for buf in bufs {
            self.write_all(buf)?;
            ret += buf.len();
        }
        Ok(ret)
    }
}

impl<T: Write> StdBypassInner<T> {
    fn new(t: T) -> Self {
        Self(LineBuffer::default(), t)
    }

    fn split(
        &mut self,
    ) -> (
        &mut LineBuffer,
        impl use<'_, T> + FnMut(&str) -> IoResult<()>,
    ) {
        let Self(lb, i) = self;
        (lb, |s| i.write_all(s.as_bytes()))
    }
}

pub struct StdoutBypass(StdBypassInner<Stdout>);

impl Default for StdoutBypass {
    fn default() -> Self {
        Self(StdBypassInner::new(stdout()))
    }
}

impl Write for StdoutBypass {
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.0.write_all(buf)
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        self.0.write_vectored(bufs)
    }
}

impl StdoutBypass {
    pub fn poll(&self) -> AnyResult<NullPollable> {
        Ok(NullPollable::new())
    }
}

pub struct StderrBypass(StdBypassInner<Stderr>);

impl Default for StderrBypass {
    fn default() -> Self {
        Self(StdBypassInner::new(stderr()))
    }
}

impl Write for StderrBypass {
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.0.write_all(buf)
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        self.0.write_vectored(bufs)
    }
}

impl StderrBypass {
    pub fn poll(&self) -> AnyResult<NullPollable> {
        Ok(NullPollable::new())
    }
}

pub type NullPollable = crate::NullPollable;
