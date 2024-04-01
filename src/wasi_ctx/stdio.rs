use std::collections::VecDeque;
use std::fmt::{Display, Result as FmtResult, Write as _};
use std::future::poll_fn;
use std::io::{Result as IoResult, Write};
use std::ops::Deref;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Poll, Waker};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use godot::prelude::*;
use memchr::memchr;
use parking_lot::{Condvar, Mutex, MutexGuard};
use wasmtime_wasi::{
    HostInputStream, HostOutputStream, StdinStream, StdoutStream, StreamError, StreamResult,
    Subscribe,
};

const BUFFER_LEN: usize = 8192;
const NL_BYTE: u8 = 10;

#[repr(transparent)]
pub struct StreamWrapper<T> {
    inner: Arc<T>,
}

impl<T> From<T> for StreamWrapper<T> {
    fn from(v: T) -> Self {
        Self { inner: Arc::new(v) }
    }
}

impl<T> Clone for StreamWrapper<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Deref for StreamWrapper<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

#[derive(Clone)]
pub struct UnbufferedWritePipe<F>(F)
where
    for<'a> F: Fn(&'a [u8]);

impl<F> UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

/*
#[async_trait]
impl<F> WasiFile for UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(FileType::CharacterDevice)
    }

    fn isatty(&self) -> bool {
        false
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Ok(FdFlags::APPEND)
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn write_vectored<'a>(&self, bufs: &[IoSlice<'a>]) -> Result<u64, Error> {
        let f = &self.0;
        let mut n = 0u64;
        for b in bufs {
            f(b);
            n += b.len() as u64;
        }

        Ok(n)
    }

    async fn write_vectored_at<'a>(
        &self,
        _bufs: &[IoSlice<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn writable(&self) -> Result<(), Error> {
        Ok(())
    }
}
*/

impl<F> StdoutStream for UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + Clone + 'static,
{
    fn stream(&self) -> Box<dyn HostOutputStream> {
        Box::new(self.clone())
    }

    fn isatty(&self) -> bool {
        false
    }
}

#[async_trait]
impl<F> Subscribe for UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    // Always ready
    async fn ready(&mut self) {}
}

impl<F> HostOutputStream for UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn write(&mut self, bytes: Bytes) -> StreamResult<()> {
        let f = &self.0;
        f(&bytes);
        Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}

pub struct LineWritePipe<F>(Mutex<InnerLineWriter<F>>)
where
    for<'a> F: Fn(&'a [u8]);

impl<F> LineWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    pub fn new(f: F) -> Self {
        Self(Mutex::new(InnerLineWriter::new(f)))
    }
}

/*
#[async_trait]
impl<F> WasiFile for LineWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(FileType::CharacterDevice)
    }

    fn isatty(&self) -> bool {
        false
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Ok(FdFlags::APPEND)
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn write_vectored<'a>(&self, bufs: &[IoSlice<'a>]) -> Result<u64, Error> {
        let mut writer = self.0.lock();
        let n = writer.write_vectored(bufs)?;

        Ok(n as _)
    }

    async fn write_vectored_at<'a>(
        &self,
        _bufs: &[IoSlice<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn writable(&self) -> Result<(), Error> {
        Ok(())
    }
}
*/

impl<F> StdoutStream for StreamWrapper<LineWritePipe<F>>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn stream(&self) -> Box<dyn HostOutputStream> {
        Box::new(self.clone())
    }

    fn isatty(&self) -> bool {
        false
    }
}

#[async_trait]
impl<F> Subscribe for StreamWrapper<LineWritePipe<F>>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    // Always ready
    async fn ready(&mut self) {}
}

impl<F> HostOutputStream for StreamWrapper<LineWritePipe<F>>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn write(&mut self, bytes: Bytes) -> StreamResult<()> {
        match self.0.lock().write(&bytes) {
            Ok(n) => debug_assert_eq!(bytes.len(), n),
            Err(e) => return Err(StreamError::LastOperationFailed(e.into())),
        }
        Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}

struct InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    buffer: Vec<u8>,
    f: F,
}

impl<F> InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    fn new(f: F) -> Self {
        let mut buffer = Vec::new();
        buffer.reserve_exact(BUFFER_LEN);
        Self { buffer, f }
    }
}

impl<F> Drop for InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    fn drop(&mut self) {
        if !self.buffer.is_empty() {
            let f = &self.f;
            f(self.buffer.as_slice())
        }
    }
}

impl<F> Write for InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn write(&mut self, mut buf: &[u8]) -> IoResult<usize> {
        let Self { buffer, f } = self;
        if buffer.len() == buffer.capacity() {
            f(buffer.as_slice());
            // SAFETY: u8 does not have Drop
            unsafe { buffer.set_len(0) };
        }

        if buf.is_empty() {
            return Ok(0);
        }

        let p = unsafe { buffer.as_mut_ptr().add(buffer.len()) };
        let mut n = 0;

        let mut l = buffer.capacity().wrapping_sub(buffer.len());
        if let Some(i) = memchr(NL_BYTE, buf) {
            let i = i.wrapping_add(1);
            if l >= i {
                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p, i);
                    let s =
                        slice::from_raw_parts(buffer.as_mut_ptr(), buffer.len().wrapping_add(i));
                    buffer.set_len(0);
                    f(s);
                }
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p, l);
                    buffer.set_len(0);
                    f(slice::from_raw_parts(
                        buffer.as_mut_ptr(),
                        buffer.capacity(),
                    ));
                    f(buf.get_unchecked(l..=i));
                }
            }

            l = i;
        } else if l >= buf.len() {
            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr(), p, buf.len());
                buffer.set_len(buffer.len().wrapping_add(buf.len()));
            }

            return Ok(buf.len());
        } else {
            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr(), p, l);
                buffer.set_len(0);
                f(slice::from_raw_parts(
                    buffer.as_mut_ptr(),
                    buffer.capacity(),
                ));
            }
        }
        (_, buf) = buf.split_at(l);
        n += l;

        while !buf.is_empty() {
            if let Some(i) = memchr(NL_BYTE, buf) {
                let (a, b) = buf.split_at(i.wrapping_add(1));
                n += a.len();
                buf = b;
                f(a);
            } else {
                if buf.len() >= buffer.capacity() {
                    let i = buf
                        .len()
                        .wrapping_sub(buf.len().wrapping_rem(buffer.capacity()));
                    let (a, b) = buf.split_at(i);
                    n += a.len();
                    buf = b;
                    f(a);
                }

                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), buffer.as_mut_ptr(), buf.len());
                    buffer.set_len(buf.len());
                }

                n += buf.len();
                break;
            }
        }

        Ok(n)
    }
}

pub struct BlockWritePipe<F>(Mutex<InnerBlockWriter<F>>)
where
    for<'a> F: Fn(&'a [u8]);

impl<F> BlockWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    pub fn new(f: F) -> Self {
        Self(Mutex::new(InnerBlockWriter::new(f)))
    }
}

/*
#[async_trait]
impl<F> WasiFile for BlockWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(FileType::CharacterDevice)
    }

    fn isatty(&self) -> bool {
        false
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Ok(FdFlags::APPEND)
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn write_vectored<'a>(&self, bufs: &[IoSlice<'a>]) -> Result<u64, Error> {
        let mut writer = self.0.lock();
        let n = writer.write_vectored(bufs)?;

        Ok(n as _)
    }

    async fn write_vectored_at<'a>(
        &self,
        _bufs: &[IoSlice<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn writable(&self) -> Result<(), Error> {
        Ok(())
    }
}
*/

impl<F> StdoutStream for StreamWrapper<BlockWritePipe<F>>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn stream(&self) -> Box<dyn HostOutputStream> {
        Box::new(self.clone())
    }

    fn isatty(&self) -> bool {
        false
    }
}

#[async_trait]
impl<F> Subscribe for StreamWrapper<BlockWritePipe<F>>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    // Always ready
    async fn ready(&mut self) {}
}

impl<F> HostOutputStream for StreamWrapper<BlockWritePipe<F>>
where
    for<'a> F: Fn(&'a [u8]) + Send + Sync + 'static,
{
    fn write(&mut self, bytes: Bytes) -> StreamResult<()> {
        match self.0.lock().write(&bytes) {
            Ok(n) => debug_assert_eq!(bytes.len(), n),
            Err(e) => return Err(StreamError::LastOperationFailed(e.into())),
        }
        Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}

struct InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    buffer: Vec<u8>,
    f: F,
}

impl<F> InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    fn new(f: F) -> Self {
        let mut buffer = Vec::new();
        buffer.reserve_exact(BUFFER_LEN);
        Self { buffer, f }
    }
}

impl<F> Drop for InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    fn drop(&mut self) {
        if !self.buffer.is_empty() {
            let f = &self.f;
            f(self.buffer.as_slice())
        }
    }
}

impl<F> Write for InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]),
{
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn write(&mut self, mut buf: &[u8]) -> IoResult<usize> {
        let Self { buffer, f } = self;
        if buffer.len() == buffer.capacity() {
            f(buffer.as_slice());
            // SAFETY: u8 does not have Drop
            unsafe { buffer.set_len(0) };
        }

        if buf.is_empty() {
            return Ok(0);
        }

        let p = unsafe { buffer.as_mut_ptr().add(buffer.len()) };
        let mut n = 0;

        let l = buffer.capacity().wrapping_sub(buffer.len());
        if l >= buf.len() {
            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr(), p, buf.len());
                buffer.set_len(buffer.len().wrapping_add(buf.len()));
            }

            return Ok(buf.len());
        } else {
            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr(), p, l);
                buffer.set_len(0);
                f(slice::from_raw_parts(
                    buffer.as_mut_ptr(),
                    buffer.capacity(),
                ));
            }
        }
        (_, buf) = buf.split_at(l);
        n += l;

        if !buf.is_empty() {
            if buf.len() >= buffer.capacity() {
                let i = buf
                    .len()
                    .wrapping_sub(buf.len().wrapping_rem(buffer.capacity()));
                let (a, b) = buf.split_at(i);
                n += a.len();
                buf = b;
                f(a);
            }

            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr(), buffer.as_mut_ptr(), buf.len());
                buffer.set_len(buf.len());
            }

            n += buf.len();
        }

        Ok(n)
    }
}

pub struct OuterStdin<F>(Arc<InnerStdin<F>>);

pub struct InnerStdin<F: ?Sized> {
    is_dropped: AtomicBool,
    cond: Condvar,
    inner: Mutex<InnerInnerStdin>,
    f: F,
}

struct InnerInnerStdin {
    buf: VecDeque<String>,
    is_eof: bool,
    ix: usize,

    waker: Option<Waker>,
}

impl<F> Drop for OuterStdin<F> {
    fn drop(&mut self) {
        self.0.is_dropped.store(true, Ordering::Release);
    }
}

impl<F: Fn()> OuterStdin<F> {
    pub fn new(f: F) -> (Self, Arc<InnerStdin<F>>) {
        let inner = Arc::new(InnerStdin {
            f,
            is_dropped: AtomicBool::new(false),
            cond: Condvar::new(),
            inner: Mutex::new(InnerInnerStdin {
                buf: VecDeque::new(),
                is_eof: false,
                ix: 0,
                waker: None,
            }),
        });
        (Self(inner.clone()), inner)
    }

    fn ensure_nonempty(&self) -> MutexGuard<'_, InnerInnerStdin> {
        let mut guard = self.0.inner.lock();
        loop {
            match &*guard {
                InnerInnerStdin { is_eof: true, .. } => break,
                InnerInnerStdin { buf, .. } if !buf.is_empty() => break,
                _ => (),
            }
            (self.0.f)();
            self.0.cond.wait(&mut guard);
        }

        guard
    }
}

impl<F: ?Sized> InnerStdin<F> {
    pub fn add_line<T: Display>(&self, line: T) -> FmtResult {
        if self.is_dropped.load(Ordering::Acquire) {
            return Ok(());
        }

        let mut guard = self.inner.lock();
        if guard.is_eof {
            return Ok(());
        }

        let buf = &mut guard.buf;
        let mut ret = String::new();
        write!(&mut ret, "{line}")?;
        if !ret.ends_with('\n') {
            ret.push('\n');
        }
        buf.push_back(ret);

        if let Some(w) = guard.waker.take() {
            w.wake();
        }
        self.cond.notify_one();
        Ok(())
    }

    pub fn close_pipe(&self) {
        if self.is_dropped.load(Ordering::Acquire) {
            return;
        }

        let mut guard = self.inner.lock();
        guard.is_eof = true;

        if let Some(w) = guard.waker.take() {
            w.wake();
        }
        self.cond.notify_one();
    }
}

/*
#[async_trait]
impl<F: Fn() + Send + Sync + 'static> WasiFile for OuterStdin<F> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(FileType::Pipe)
    }

    fn isatty(&self) -> bool {
        false
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Ok(FdFlags::empty())
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn read_vectored<'a>(&self, bufs: &mut [IoSliceMut<'a>]) -> Result<u64, Error> {
        let mut inner = self.ensure_nonempty();
        let InnerInnerStdin { buf, ix, is_eof } = &mut *inner;
        if *is_eof {
            return Ok(0);
        }

        let mut n = 0u64;
        for b in bufs {
            if b.len() == 0 {
                continue;
            } else if *ix >= buf.len() {
                break;
            }

            let l = b.len().min(buf.len() - *ix);
            b[..l].copy_from_slice(&buf.as_bytes()[*ix..*ix + l]);
            n += l as u64;
            *ix += l;
        }

        if *ix >= buf.len() {
            buf.clear();
            *ix = 0;
        }

        Ok(n)
    }

    async fn read_vectored_at<'a>(
        &self,
        _bufs: &mut [std::io::IoSliceMut<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn readable(&self) -> Result<(), Error> {
        Ok(())
    }
}
*/

impl<F: Fn() + Send + Sync + 'static> StdinStream for OuterStdin<F> {
    fn stream(&self) -> Box<dyn HostInputStream> {
        Box::new(Self(self.0.clone()))
    }

    fn isatty(&self) -> bool {
        false
    }
}

#[async_trait]
impl<F: Fn() + Send + Sync + 'static> Subscribe for OuterStdin<F> {
    async fn ready(&mut self) {
        poll_fn(|cx| {
            let mut guard = self.0.inner.lock();

            if match &*guard {
                InnerInnerStdin { is_eof: true, .. } => true,
                InnerInnerStdin { buf, .. } if !buf.is_empty() => true,
                _ => false,
            } {
                return Poll::Ready(());
            }

            (self.0.f)();
            guard.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await
    }
}

impl<F: Fn() + Send + Sync + 'static> HostInputStream for OuterStdin<F> {
    fn read(&mut self, mut size: usize) -> StreamResult<Bytes> {
        let mut inner = self.ensure_nonempty();
        let InnerInnerStdin {
            buf, ix, is_eof, ..
        } = &mut *inner;
        if *is_eof && buf.is_empty() {
            return Err(StreamError::Closed);
        }

        let mut ret = BytesMut::with_capacity(size);
        while size > 0 {
            let Some(v) = buf.front() else { break };

            if v.is_empty() {
                buf.pop_front();
                *ix = 0;
            } else if v.len() - *ix <= size {
                let s = &v.as_bytes()[*ix..];
                ret.extend_from_slice(s);
                size -= s.len();
                buf.pop_front();
                *ix = 0;
            } else {
                ret.extend_from_slice(&v.as_bytes()[*ix..*ix + size]);
                *ix += size;
                size = 0;
            }
        }

        Ok(ret.into())
    }

    fn skip(&mut self, mut size: usize) -> StreamResult<usize> {
        let mut inner = self.ensure_nonempty();
        let InnerInnerStdin {
            buf, ix, is_eof, ..
        } = &mut *inner;
        if *is_eof && buf.is_empty() {
            return Err(StreamError::Closed);
        }

        let mut ret = 0;
        while size > 0 {
            let Some(v) = buf.front() else { break };

            if v.is_empty() {
                buf.pop_front();
                *ix = 0;
            } else if v.len() - *ix <= size {
                let s = &v.as_bytes()[*ix..];
                ret += s.len();
                size -= s.len();
                buf.pop_front();
                *ix = 0;
            } else {
                ret += size;
                *ix += size;
                size = 0;
            }
        }

        Ok(ret)
    }
}

pub struct ByteBufferReadPipe {
    buf: Vec<u8>,
    pos: Mutex<usize>,
}

impl ByteBufferReadPipe {
    pub fn new(buf: PackedByteArray) -> Self {
        Self {
            buf: buf.to_vec(),
            pos: Mutex::new(0),
        }
    }
}

/*
#[async_trait]
impl WasiFile for ByteBufferReadPipe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(FileType::Pipe)
    }

    fn isatty(&self) -> bool {
        false
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Ok(FdFlags::empty())
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn read_vectored<'a>(&self, bufs: &mut [IoSliceMut<'a>]) -> Result<u64, Error> {
        let buf = &self.buf;
        let mut pos = self.pos.lock();

        if *pos == buf.len() {
            return Ok(0);
        }

        let mut n = 0u64;
        for b in bufs {
            if b.len() == 0 {
                continue;
            }

            let l = b.len().min(buf.len() - *pos);
            b[..l].copy_from_slice(&buf[*pos..*pos + l]);
            n += l as u64;
            *pos += l;

            if *pos == buf.len() {
                break;
            }
        }

        Ok(n)
    }

    async fn read_vectored_at<'a>(
        &self,
        _bufs: &mut [std::io::IoSliceMut<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn readable(&self) -> Result<(), Error> {
        Ok(())
    }
}
*/

impl StdinStream for StreamWrapper<ByteBufferReadPipe> {
    fn stream(&self) -> Box<dyn HostInputStream> {
        Box::new(self.clone())
    }

    fn isatty(&self) -> bool {
        false
    }
}

#[async_trait]
impl Subscribe for StreamWrapper<ByteBufferReadPipe> {
    // Always ready
    async fn ready(&mut self) {}
}

impl HostInputStream for StreamWrapper<ByteBufferReadPipe> {
    fn read(&mut self, size: usize) -> StreamResult<Bytes> {
        let buf = &self.buf;
        let mut pos = self.pos.lock();

        if *pos == buf.len() {
            return Err(StreamError::Closed);
        } else if size == 0 {
            return Ok(Bytes::new());
        }

        let s;
        if buf.len() - *pos >= size {
            s = &buf[*pos..*pos + size];
            *pos += size;
        } else {
            s = &buf[*pos..];
            *pos += buf.len();
        }
        Ok(BytesMut::from(s).into())
    }

    fn skip(&mut self, size: usize) -> StreamResult<usize> {
        let buf = &self.buf;
        let mut pos = self.pos.lock();

        if *pos == buf.len() {
            return Err(StreamError::Closed);
        } else if size == 0 {
            return Ok(0);
        }

        let s;
        if buf.len() - *pos >= size {
            s = size;
            *pos += size;
        } else {
            s = buf.len() - *pos;
            *pos += buf.len();
        }
        Ok(s)
    }
}
