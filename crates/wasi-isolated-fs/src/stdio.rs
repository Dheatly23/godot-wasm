use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::{
    Error as IoError, ErrorKind, IoSlice, Result as IoResult, Stderr, Stdout, Write, stderr, stdout,
};
use std::mem::replace;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::ptr::null;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result as AnyResult;
use cfg_if::cfg_if;
use memchr::memchr_iter;
use parking_lot::{Condvar, Mutex};
use scopeguard::{defer, guard, guard_on_unwind};
use smallvec::SmallVec;
use tracing::instrument;

use crate::poll::WaitData;

const MAX_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct NullStdio {
    _p: (),
}

impl Debug for NullStdio {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "NullStdio")
    }
}

pub struct StdinSignal {
    pub(crate) inner: Mutex<StdinInner>,
    pub(crate) cond: Condvar,
    f: Box<dyn Fn() + Send + Sync>,
}

impl Debug for StdinSignal {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("StdinSignal")
            .field(&(self as *const _))
            .finish()
    }
}

#[derive(Debug)]
pub struct StdinProvider(Arc<StdinSignal>);

type StdinInnerData = SmallVec<[u8; 32]>;

pub(crate) struct StdinInner {
    closed: bool,
    data: StdinInnerData,
    start: usize,
    end: usize,

    pub(crate) head: *const WaitData,
}

unsafe impl Send for StdinInner {}
unsafe impl Sync for StdinInner {}
impl UnwindSafe for StdinInner {}
impl RefUnwindSafe for StdinInner {}

impl Debug for StdinInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("StdinInner")
            .field("closed", &self.closed)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("len", &self.len())
            .field("has_waiting", &!self.head.is_null())
            .finish_non_exhaustive()
    }
}

impl StdinInner {
    fn new() -> Self {
        StdinInner {
            closed: false,
            data: StdinInnerData::from_buf(Default::default()),
            start: 0,
            end: 0,

            head: null(),
        }
    }

    fn len(&self) -> usize {
        (self.end as isize - self.start as isize).wrapping_rem_euclid(self.data.len() as isize)
            as usize
    }

    #[instrument(skip(buf), fields(buf.len = buf.len()))]
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

    #[instrument]
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

    pub(crate) fn is_ready(&self) -> bool {
        self.closed || self.end != self.start
    }

    #[instrument]
    fn notify(&mut self) {
        let mut p = replace(&mut self.head, null());
        while !p.is_null() {
            // SAFETY: Signal is locked, so all nodes are held.
            unsafe {
                let n = *(*p).next.get();
                (*p).waited();
                p = n;
            }
        }
    }
}

impl StdinSignal {
    pub fn new(f: Box<dyn Fn() + Send + Sync>) -> (Arc<Self>, StdinProvider) {
        let ret = Arc::new(Self {
            inner: Mutex::new(StdinInner::new()),
            cond: Condvar::new(),
            f,
        });

        (ret.clone(), StdinProvider(ret))
    }

    pub fn is_ready(&self) -> bool {
        self.inner.lock().is_ready()
    }

    #[instrument]
    pub fn read(&self, len: usize) -> IoResult<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }

        let mut guard = self.inner.lock();

        let (a, b) = guard.pop_data(len);

        let mut ret = vec![0u8; a.len() + b.len()];
        ret[..a.len()].copy_from_slice(a);
        ret[a.len()..].copy_from_slice(b);
        Ok(ret)
    }

    #[instrument]
    pub fn read_block(&self, len: usize, timeout: Option<Instant>) -> IoResult<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }

        let mut t = Instant::now() + MAX_TIMEOUT;
        if let Some(v) = timeout {
            t = t.min(v);
        }
        let mut guard = self.inner.lock();

        let (a, b) = loop {
            let closed = guard.closed;
            let (a, b) = guard.pop_data(len);
            if !closed && a.is_empty() && b.is_empty() {
                (self.f)();
                if self.cond.wait_until(&mut guard, t).timed_out() {
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

    #[instrument]
    pub fn skip(&self, len: usize) -> IoResult<usize> {
        if len == 0 {
            return Ok(0);
        }

        let mut guard = self.inner.lock();

        let (a, b) = guard.pop_data(len);
        Ok(a.len() + b.len())
    }

    #[instrument]
    pub fn skip_block(&self, len: usize, timeout: Option<Instant>) -> IoResult<usize> {
        if len == 0 {
            return Ok(0);
        }

        let mut t = Instant::now() + MAX_TIMEOUT;
        if let Some(v) = timeout {
            t = t.min(v);
        }
        let mut guard = self.inner.lock();

        let (a, b) = loop {
            let closed = guard.closed;
            let (a, b) = guard.pop_data(len);
            if !closed && a.is_empty() && b.is_empty() {
                (self.f)();
                if self.cond.wait_until(&mut guard, t).timed_out() {
                    return Err(ErrorKind::TimedOut.into());
                }
            } else {
                break (a, b);
            }
        };

        Ok(a.len() + b.len())
    }

    #[instrument]
    pub fn poll(self: &Arc<Self>) -> AnyResult<StdinSignalPollable> {
        Ok(StdinSignalPollable(self.clone()))
    }
}

impl StdinProvider {
    pub fn dup(&self) -> Self {
        Self(self.0.clone())
    }

    pub fn consumer(&self) -> Arc<StdinSignal> {
        self.0.clone()
    }

    pub fn is_paired(&self, other: &Arc<StdinSignal>) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    #[instrument(skip(buf), fields(buf.len = buf.len()))]
    pub fn write(&self, buf: &[u8]) {
        let mut guard = self.0.inner.lock();
        if guard.closed {
            return;
        }
        guard.push_data(buf);
        guard.notify();
        self.0.cond.notify_one();
    }

    #[instrument]
    pub fn close(&self) {
        let mut guard = self.0.inner.lock();
        guard.closed = true;
        guard.notify();
        self.0.cond.notify_one();
    }
}

#[derive(Debug)]
pub struct StdinSignalPollable(pub(crate) Arc<StdinSignal>);

impl StdinSignalPollable {
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        self.0.is_ready()
    }

    #[instrument]
    pub fn block(&self, timeout: Option<Instant>) -> AnyResult<()> {
        let mut t = Instant::now() + MAX_TIMEOUT;
        if let Some(v) = timeout {
            t = t.min(v);
        }
        let mut guard = self.0.inner.lock();
        while !guard.closed && guard.end == guard.start {
            if self.0.cond.wait_until(&mut guard, t).timed_out() {
                return Err(IoError::from(ErrorKind::TimedOut).into());
            }
        }

        Ok(())
    }
}

pub trait HostStdin: Debug {
    fn read(&self, len: usize) -> IoResult<Vec<u8>>;
    fn read_block(&self, len: usize, timeout: Option<Instant>) -> IoResult<Vec<u8>>;
    fn skip(&self, len: usize) -> IoResult<usize>;
    fn skip_block(&self, len: usize, timeout: Option<Instant>) -> IoResult<usize>;
    fn block(&self, timeout: Option<Instant>) -> IoResult<()>;
}

pub trait HostStdout: Debug {
    fn write(&self, buf: &[u8]) -> IoResult<()>;
    fn flush(&self) -> IoResult<()>;
}

cfg_if! {
    if #[cfg(test)] {
        const BUF_LEN: usize = 256;
    } else {
        const BUF_LEN: usize = 1024;
    }
}

pub struct LineBuffer {
    buf: Box<[u8; BUF_LEN]>,
    len: usize,
    s: String,
}

const LINESEP: u8 = b'\n';
const REPLACEMENT: &str = "\u{FFFD}";

impl Default for LineBuffer {
    fn default() -> Self {
        Self {
            buf: Box::new([0; BUF_LEN]),
            len: 0,
            s: String::new(),
        }
    }
}

impl Debug for LineBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("LineBuffer")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl LineBuffer {
    pub fn write<F, E>(&mut self, mut f: F, data: &[u8]) -> Result<(), E>
    where
        for<'a> F: FnMut(&'a str) -> Result<(), E>,
    {
        let Self { buf, len, s } = &mut *guard_on_unwind(self, |this| this.len = 0);

        let mut i = 0;
        let mut it = memchr_iter(LINESEP, data);
        while i < data.len() {
            let e = it.next().map_or(data.len(), |e| e + 1);
            let mut d = &data[i..e];
            i = e;

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

pub type StdoutCbLineFn = Box<dyn Send + Sync + FnMut(&str)>;

#[derive(Debug)]
pub struct StdoutCbLineBuffered(Mutex<StdoutCbLineBufferedInner>);

struct StdoutCbLineBufferedInner {
    buf: LineBuffer,
    cb: StdoutCbLineFn,
}

impl Debug for StdoutCbLineBufferedInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("StdoutCbLineBufferedInner")
            .field("buf", &self.buf)
            .finish_non_exhaustive()
    }
}

impl StdoutCbLineBuffered {
    pub fn new(cb: StdoutCbLineFn) -> Self {
        Self(Mutex::new(StdoutCbLineBufferedInner {
            buf: Default::default(),
            cb,
        }))
    }
}

impl HostStdout for StdoutCbLineBuffered {
    #[instrument(skip(buf), fields(buf.len = buf.len()))]
    fn write(&self, buf: &[u8]) -> IoResult<()> {
        let mut g = self.0.lock();
        let (lb, f) = g.split();
        lb.write(f, buf)
    }

    #[instrument]
    fn flush(&self) -> IoResult<()> {
        /* Rust asks to flush every time and messes up the buffering mechanism.
        let mut g = self.0.lock();
        let (lb, f) = g.split();
        lb.flush(f)
        */
        Ok(())
    }
}

impl StdoutCbLineBufferedInner {
    fn split(&mut self) -> (&mut LineBuffer, impl use<'_> + FnMut(&str) -> IoResult<()>) {
        let Self { buf, cb } = self;
        (buf, |s| {
            cb(s);
            Ok(())
        })
    }
}

pub type StdoutCbBlockFn = Box<dyn Send + Sync + FnMut(&[u8])>;

#[derive(Debug)]
pub struct StdoutCbBlockBuffered(Mutex<StdoutCbBlockBufferedInner>);

struct StdoutCbBlockBufferedInner {
    buf: Box<[u8; BUF_LEN]>,
    len: usize,
    cb: StdoutCbBlockFn,
}

impl Debug for StdoutCbBlockBufferedInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("StdoutCbBlockBufferedInner")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl StdoutCbBlockBuffered {
    pub fn new(cb: StdoutCbBlockFn) -> Self {
        Self(Mutex::new(StdoutCbBlockBufferedInner {
            buf: Box::new([0; BUF_LEN]),
            len: 0,
            cb,
        }))
    }
}

impl HostStdout for StdoutCbBlockBuffered {
    #[instrument(skip(buf), fields(buf.len = buf.len()))]
    fn write(&self, mut buf: &[u8]) -> IoResult<()> {
        let this = &mut *self.0.lock();
        let cb = &mut this.cb;
        while !buf.is_empty() {
            if this.len == 0 && buf.len() >= this.buf.len() {
                let a;
                (a, buf) = buf.split_at(this.buf.len());
                cb(a);
                continue;
            }

            let i = this.buf.len() - this.len;
            if i >= buf.len() {
                let e = this.len + buf.len();
                this.buf[this.len..e].copy_from_slice(buf);
                (this.len, buf) = (e, &[]);
            } else {
                let a;
                (a, buf) = buf.split_at(i);
                this.buf[this.len..].copy_from_slice(a);
                this.len = this.buf.len();
            }

            debug_assert!(
                this.len <= this.buf.len(),
                "{} > {}",
                this.len,
                this.buf.len()
            );
            if this.len >= this.buf.len() {
                let len = &mut this.len;
                defer! {
                    *len = 0;
                }
                cb(&this.buf[..]);
            }
        }

        Ok(())
    }

    #[instrument]
    fn flush(&self) -> IoResult<()> {
        /*
        let this = &mut **guard(self.0.lock(), |mut this| this.len = 0);
        if this.len > 0 {
            (this.cb)(&this.buf[..this.len]);
        }
        */
        Ok(())
    }
}

#[derive(Debug)]
struct StdBypassInner<T: Debug>(LineBuffer, T);

impl<T: Debug + Write> Write for StdBypassInner<T> {
    #[instrument(skip(buf), fields(buf.len = buf.len()))]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        let (lb, f) = self.split();
        lb.write(f, buf)
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    #[instrument]
    fn flush(&mut self) -> IoResult<()> {
        let (lb, f) = self.split();
        lb.flush(f)?;
        self.1.flush()
    }

    #[instrument(skip(bufs), fields(bufs.len = bufs.len()))]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IoResult<usize> {
        let mut ret = 0;
        for buf in bufs {
            self.write_all(buf)?;
            ret += buf.len();
        }
        Ok(ret)
    }
}

impl<T: Debug + Write> StdBypassInner<T> {
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

#[derive(Debug)]
pub struct StdoutBypass(Mutex<StdBypassInner<Stdout>>);

impl Default for StdoutBypass {
    fn default() -> Self {
        Self(Mutex::new(StdBypassInner::new(stdout())))
    }
}

impl HostStdout for StdoutBypass {
    #[instrument(skip(buf), fields(buf.len = buf.len()))]
    fn write(&self, buf: &[u8]) -> IoResult<()> {
        self.0.lock().write_all(buf)
    }

    #[instrument]
    fn flush(&self) -> IoResult<()> {
        self.0.lock().flush()
    }
}

#[derive(Debug)]
pub struct StderrBypass(Mutex<StdBypassInner<Stderr>>);

impl Default for StderrBypass {
    fn default() -> Self {
        Self(Mutex::new(StdBypassInner::new(stderr())))
    }
}

impl HostStdout for StderrBypass {
    #[instrument(skip(buf), fields(buf.len = buf.len()))]
    fn write(&self, buf: &[u8]) -> IoResult<()> {
        self.0.lock().write_all(buf)
    }

    #[instrument]
    fn flush(&self) -> IoResult<()> {
        self.0.lock().flush()
    }
}

pub type NullPollable = crate::NullPollable;

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeSet;

    use anyhow::Result as AnyResult;
    use proptest::collection::{SizeRange, btree_set, vec};
    use proptest::prelude::*;

    #[derive(Debug, Clone)]
    enum PushPop {
        Push(Vec<u8>),
        Pop(usize),
    }

    fn push_pop_strategy(
        n: impl Clone + Strategy<Value = usize> + Into<SizeRange>,
    ) -> impl Strategy<Value = PushPop> {
        prop_oneof![
            n.clone().prop_map(PushPop::Pop),
            vec(any::<u8>(), n).prop_map(PushPop::Push),
        ]
    }

    #[test]
    fn test_stdin_inner_rw() {
        fn f(v: Vec<Vec<u8>>) {
            let mut inner = StdinInner::new();

            for src in v {
                assert_eq!(inner.len(), 0);
                inner.push_data(&src);
                let mut dst = vec![0; src.len()];
                assert_eq!(inner.len(), src.len());
                let (a, b) = inner.pop_data(dst.len());
                dst[..a.len()].copy_from_slice(a);
                dst[a.len()..].copy_from_slice(b);
                assert_eq!(src, dst);
                assert_eq!(inner.len(), 0);
            }
        }

        proptest!(|(v in vec(vec(any::<u8>(), 256), 0..16))| f(v));
    }

    #[test]
    fn test_stdin_inner_rw_uneq() {
        fn f(v: Vec<PushPop>) {
            let mut inner = StdinInner::new();
            let mut buf = Vec::new();

            for i in v {
                match i {
                    PushPop::Push(v) => {
                        inner.push_data(&v);
                        buf.extend_from_slice(&v);
                        assert_eq!(buf.len(), inner.len());
                    }
                    PushPop::Pop(mut n) => {
                        n = n.min(buf.len());
                        let mut src = buf.split_off(n);
                        (src, buf) = (buf, src);
                        let mut dst = vec![0; n];
                        let (a, b) = inner.pop_data(n);
                        dst[..a.len()].copy_from_slice(a);
                        dst[a.len()..].copy_from_slice(b);
                        assert_eq!(src, dst);
                        assert_eq!(buf.len(), inner.len());
                    }
                }
            }
        }

        proptest!(|(v in vec(push_pop_strategy(0..256usize), 0..16))| f(v));
    }

    #[test]
    fn test_line_buf_rw() {
        fn f(s: String, seg: BTreeSet<usize>) {
            let mut buf = LineBuffer::default();

            let mut p = 0;
            let mut f = |v: &str| -> AnyResult<()> {
                assert_eq!(v.as_bytes(), &s.as_bytes()[p..p + v.len()], "p: {p}");
                p += v.len();
                Ok(())
            };

            let mut i = 0;
            for e in seg {
                let v = &s.as_bytes()[i..e];
                buf.write(&mut f, v).unwrap();
                i = e;
            }

            if i < s.len() {
                buf.write(&mut f, &s.as_bytes()[i..]).unwrap();
            }
            buf.flush(f).unwrap();
            assert_eq!(p, s.len());
        }

        proptest!(|((seg, s) in "([^\n]{0,64}\n?){0,16}".prop_flat_map(|s| (btree_set(0..=s.len(), 0..16), Just(s))))| f(s, seg));
    }
}
