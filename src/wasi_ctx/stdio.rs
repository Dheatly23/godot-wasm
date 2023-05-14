use std::io::{IoSlice, Result as IoResult, SeekFrom, Write};
use std::ptr;
use std::slice;

use async_trait::async_trait;
use memchr::memchr;
use parking_lot::Mutex;
use wasi_common::file::{FdFlags, FileType, WasiFile};
use wasi_common::{Error, ErrorExt};

pub struct WritePipe<F>(Mutex<InnerWriter<F>>)
where
    for<'a> F: Fn(&'a [u8]) -> ();

impl<F> WritePipe<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    pub fn new(f: F) -> Self {
        Self(Mutex::new(InnerWriter::new(f)))
    }
}

#[async_trait]
impl<F> WasiFile for WritePipe<F>
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
}

struct InnerWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    buffer: Vec<u8>,
    f: F,
}

const BUFFER_LEN: usize = 8192;
const NL_BYTE: u8 = 10;

impl<F> InnerWriter<F>
where
    for<'a> F: Fn(&'a [u8]) -> (),
{
    fn new(f: F) -> Self {
        let mut buffer = Vec::new();
        buffer.reserve_exact(BUFFER_LEN);
        Self { buffer, f }
    }
}

impl<F> Drop for InnerWriter<F>
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

impl<F> Write for InnerWriter<F>
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

        let p = buffer.as_mut_ptr();
        let mut n = 0;

        let mut l = buffer.capacity().wrapping_sub(buffer.len());
        if let Some(i) = memchr(NL_BYTE, buf) {
            let i = i.wrapping_add(1);
            if l >= i {
                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p.add(buffer.len()), i);
                    buffer.set_len(buffer.len().wrapping_add(i));
                    f(buffer.as_slice());
                    buffer.set_len(0);
                }
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p.add(buffer.len()), l);
                    buffer.set_len(0);
                    f(slice::from_raw_parts(p, buffer.capacity()));
                    f(buf.get_unchecked(l..=i));
                }
            }

            l = i;
        } else {
            if l >= buf.len() {
                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p.add(buffer.len()), buf.len());
                    buffer.set_len(buffer.len().wrapping_add(buf.len()));
                }

                return Ok(buf.len());
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p.add(buffer.len()), l);
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
                while buf.len() > buffer.capacity() {
                    let (a, b) = buf.split_at(buffer.capacity());
                    n += buffer.capacity();
                    buf = b;
                    f(a);
                }

                unsafe {
                    ptr::copy_nonoverlapping(buf.as_ptr(), p, buf.len());
                    buffer.set_len(buf.len());
                }

                n += buf.len();
                break;
            }
        }

        Ok(n)
    }
}
