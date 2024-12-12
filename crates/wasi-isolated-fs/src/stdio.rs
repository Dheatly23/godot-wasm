use std::sync::Arc;

use memchr::memchr;
use parking_lot::{Condvar, Mutex};
use scopeguard::{guard, ScopeGuard};
use smallvec::SmallVec;

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
            self.end = i;
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
            let i = len.min(self.end);
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

    pub fn read(&self, len: usize) -> Vec<u8> {
        let mut guard = self.inner.lock();

        let (a, b) = loop {
            let closed = guard.closed;
            let (a, b) = guard.pop_data(len);
            if !closed && a.is_empty() && b.is_empty() {
                (self.f)();
                self.cond.wait(&mut guard);
            } else {
                break (a, b);
            }
        };

        let mut ret = vec![0u8; a.len() + b.len()];
        ret[..a.len()].copy_from_slice(a);
        ret[a.len()..].copy_from_slice(b);
        ret
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
        guard.push_data(buf);
        self.0.cond.notify_one();
    }

    pub fn close(&self) {
        let mut guard = self.0.inner.lock();
        guard.closed = true;
        self.0.cond.notify_one();
    }
}

pub struct StdoutCb<F: ?Sized + Send + Sync + FnMut(&[u8])>(F);

impl<F: Send + Sync + Fn(&[u8])> StdoutCb<F> {
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

impl<F: ?Sized + Send + Sync + Fn(&[u8])> StdoutCb<F> {
    pub fn write(&self, buf: &[u8]) {
        (self.0)(buf)
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
        Self::new()
    }
}

impl LineBuffer {
    pub fn new() -> Self {
        Self {
            buf: Box::new([0; 1024]),
            len: 0,
            s: String::new(),
        }
    }

    pub fn write<F>(&mut self, mut f: F, mut data: &[u8])
    where
        for<'a> F: FnMut(&'a str),
    {
        let mut g = guard(self, |this| this.len = 0);
        let this: &mut Self = &mut g;

        while !data.is_empty() {
            let mut d;
            (d, data) = if let Some(i) = memchr(LINESEP, data) {
                data.split_at(i + 1)
            } else {
                (data, &[] as &[_])
            };

            while !d.is_empty() {
                if this.len == 0 && *d.last().unwrap() == LINESEP {
                    // Emit line without buffering
                    let mut it = d.utf8_chunks();
                    let chunk = it.next().unwrap();
                    if chunk.invalid().is_empty() {
                        f(chunk.valid());
                        d = &[];
                        continue;
                    }

                    this.s.clear();
                    this.s.reserve(d.len());
                    this.s += chunk.valid();
                    this.s += REPLACEMENT;
                    for chunk in it {
                        this.s += chunk.valid();
                        if !chunk.invalid().is_empty() {
                            this.s += REPLACEMENT;
                        }
                    }

                    f(&this.s);
                    d = &[];
                    continue;
                }

                let rest = this.buf.len() - this.len;
                if rest >= d.len() {
                    this.buf[this.len..this.len + d.len()].copy_from_slice(d);
                    this.len += d.len();
                    d = &[];
                } else {
                    let a;
                    (a, d) = d.split_at(rest);
                    this.buf[this.len..].copy_from_slice(a);
                    this.len = this.buf.len();
                }

                if this.buf[this.len - 1] == LINESEP {
                    // Emit the line
                    let mut it = this.buf[..this.len].utf8_chunks();
                    let chunk = it.next().unwrap();
                    if chunk.invalid().is_empty() {
                        f(chunk.valid());
                    } else {
                        this.s.clear();
                        this.s.reserve(this.len);
                        this.s += chunk.valid();
                        this.s += REPLACEMENT;
                        for chunk in it {
                            this.s += chunk.valid();
                            if !chunk.invalid().is_empty() {
                                this.s += REPLACEMENT;
                            }
                        }

                        f(&this.s);
                    }
                    this.len = 0;
                    continue;
                } else if this.len < this.buf.len() {
                    continue;
                }

                // Buffer full
                let mut it = this.buf.utf8_chunks();
                let chunk = it.next().unwrap();
                let i = chunk.invalid().len();
                if i == 0 {
                    // All of it is valid
                    f(chunk.valid());
                    this.len = 0;
                    continue;
                } else if chunk.valid().len() + i == this.buf.len() {
                    // Some invalid residual in buffer
                    f(chunk.valid());
                    let s = this.buf.len() - i;
                    this.buf.copy_within(s.., 0);
                    this.len = i;
                    continue;
                }

                this.s.clear();
                this.s.reserve(this.buf.len());
                this.s += chunk.valid();
                this.s += REPLACEMENT;
                let mut chunk = it.next();
                while let Some(c) = chunk {
                    this.s += c.valid();
                    let i = c.invalid().len();
                    chunk = it.next();

                    if chunk.is_none() {
                        if i != 0 {
                            // Some invalid residual in buffer
                            let s = this.buf.len() - i;
                            this.buf.copy_within(s.., 0);
                        }
                        this.len = i;
                        break;
                    } else if i != 0 {
                        this.s += REPLACEMENT;
                    }
                }

                f(&this.s);
            }
        }

        // Defuse the guard
        ScopeGuard::into_inner(g);
    }
}
