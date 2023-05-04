use std::io::{IoSlice, LineWriter, Result as IoResult, SeekFrom, Write};

use async_trait::async_trait;
use godot::prelude::*;
use parking_lot::Mutex;
use wasi_common::file::{FdFlags, FileType, WasiFile};
use wasi_common::{Error, ErrorExt};

use crate::wasi_ctx::WasiContext;
use crate::wasm_util::SendSyncWrapper;

pub struct ContextStdout {
    writer: Mutex<LineWriter<ContextStdoutInner>>,
}

#[allow(dead_code)]
struct ContextStdoutInner {
    base: SendSyncWrapper<Gd<WasiContext>>,
}

impl ContextStdout {
    pub fn new(base: Gd<WasiContext>) -> Self {
        Self {
            writer: Mutex::new(LineWriter::new(ContextStdoutInner {
                base: SendSyncWrapper::new(base),
            })),
        }
    }
}

#[async_trait]
impl WasiFile for ContextStdout {
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
        let mut writer = self.writer.lock();
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

impl Write for ContextStdoutInner {
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn write_all(&mut self, _buf: &[u8]) -> IoResult<()> {
        // TODO: Signal not supported yet!
        /*
        unsafe {
            self.base.assume_safe().emit_signal(
                "stdout_emit",
                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
            );
        }
        */
        Ok(())
    }
}

pub struct ContextStderr {
    writer: Mutex<LineWriter<ContextStderrInner>>,
}

#[allow(dead_code)]
struct ContextStderrInner {
    base: SendSyncWrapper<Gd<WasiContext>>,
}

impl ContextStderr {
    pub fn new(base: Gd<WasiContext>) -> Self {
        Self {
            writer: Mutex::new(LineWriter::new(ContextStderrInner {
                base: SendSyncWrapper::new(base),
            })),
        }
    }
}

#[async_trait]
impl WasiFile for ContextStderr {
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
        let mut writer = self.writer.lock();
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

impl Write for ContextStderrInner {
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn write_all(&mut self, _buf: &[u8]) -> IoResult<()> {
        // TODO: Signal not supported yet!
        /*
        unsafe {
            self.base.assume_safe().emit_signal(
                "stderr_emit",
                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
            );
        }
        */
        Ok(())
    }
}
