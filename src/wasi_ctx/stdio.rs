use std::fmt::{Display, Result as FmtResult, Write as _};
use std::io::{IoSlice, IoSliceMut, Result as IoResult, SeekFrom, Write};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use memchr::memchr;
use parking_lot::{Condvar, Mutex, MutexGuard};
use wasi_common::file::{FdFlags, FileType, WasiFile};
use wasi_common::{Error, ErrorExt};

const BUFFER_LEN: usize = 8192;
const NL_BYTE: u8 = 10;

pub struct UnbufferedWritePipe<F>(F)
where
    for<'a> F: Fn(&'a [u8]) -> ();

impl<F> UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

#[async_trait]
impl<F> WasiFile for UnbufferedWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> () + Send + Sync + 'static,
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

pub struct LineWritePipe<F>(Mutex<InnerLineWriter<F>>)
where
    for<'a> F: Fn(&'a [u8]) -> ();

impl<F> LineWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    pub fn new(f: F) -> Self {
        Self(Mutex::new(InnerLineWriter::new(f)))
    }
}

#[async_trait]
impl<F> WasiFile for LineWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> () + Send + Sync + 'static,
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

struct InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    buffer: Vec<u8>,
    f: F,
}

impl<F> InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    fn new(f: F) -> Self {
        let mut buffer = Vec::new();
        buffer.reserve_exact(BUFFER_LEN);
        Self { buffer, f }
    }
}

impl<F> Drop for InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    fn drop(&mut self) {
        if self.buffer.len() > 0 {
            let f = &self.f;
            f(self.buffer.as_slice())
        }
    }
}

impl<F> Write for InnerLineWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
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

        if buf.len() == 0 {
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
        } else {
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
                    f(slice::from_raw_parts(p, buffer.capacity()));
                }
            }
        }
        (_, buf) = buf.split_at(l);
        n += l;

        while buf.len() > 0 {
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
    for<'a> F: Fn(&'a [u8]) -> ();

impl<F> BlockWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    pub fn new(f: F) -> Self {
        Self(Mutex::new(InnerBlockWriter::new(f)))
    }
}

#[async_trait]
impl<F> WasiFile for BlockWritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> () + Send + Sync + 'static,
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

struct InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    buffer: Vec<u8>,
    f: F,
}

impl<F> InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    fn new(f: F) -> Self {
        let mut buffer = Vec::new();
        buffer.reserve_exact(BUFFER_LEN);
        Self { buffer, f }
    }
}

impl<F> Drop for InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    fn drop(&mut self) {
        if self.buffer.len() > 0 {
            let f = &self.f;
            f(self.buffer.as_slice())
        }
    }
}

impl<F> Write for InnerBlockWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
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

        if buf.len() == 0 {
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
                f(slice::from_raw_parts(p, buffer.capacity()));
            }
        }
        (_, buf) = buf.split_at(l);
        n += l;

        if buf.len() > 0 {
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
    buf: Option<String>,
    ix: usize,
}

impl<F> Drop for OuterStdin<F> {
    fn drop(&mut self) {
        self.0.is_dropped.store(true, Ordering::Release);
    }
}

impl<F: Fn() -> ()> OuterStdin<F> {
    pub fn new(f: F) -> (Self, Arc<InnerStdin<F>>) {
        let inner = Arc::new(InnerStdin {
            f,
            is_dropped: AtomicBool::new(false),
            cond: Condvar::new(),
            inner: Mutex::new(InnerInnerStdin {
                buf: Some(String::new()),
                ix: 0,
            }),
        });
        (Self(inner.clone()), inner)
    }

    fn ensure_nonempty(&self) -> MutexGuard<'_, InnerInnerStdin> {
        let mut guard = self.0.inner.lock();
        loop {
            match &mut *guard {
                InnerInnerStdin { buf: None, .. } => break,
                InnerInnerStdin { buf: Some(b), ix } if *ix >= b.len() => break,
                _ => (),
            }
            (&self.0.f)();
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
        let Some(buf) = &mut guard.buf else { return Ok(()) };
        let ret = write!(&mut *buf, "{}", line);
        if buf.chars().last() != Some('\n') {
            buf.push('\n');
        }

        self.cond.notify_one();
        ret
    }
}

#[async_trait]
impl<F: Fn() -> () + Send + Sync + 'static> WasiFile for OuterStdin<F> {
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
        Ok(FdFlags::APPEND)
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }

    async fn read_vectored<'a>(&self, bufs: &mut [IoSliceMut<'a>]) -> Result<u64, Error> {
        let mut inner = self.ensure_nonempty();
        let InnerInnerStdin { buf: Some(buf), ix } = &mut *inner else { return Ok(0) };

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
