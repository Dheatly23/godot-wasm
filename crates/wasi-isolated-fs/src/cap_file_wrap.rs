#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::Write;
use std::io::{ErrorKind, Result as IoResult};

#[cfg(windows)]
use cap_fs_ext::OpenOptions;
#[cfg(windows)]
use cap_fs_ext::Reopen;
use cap_std::fs::{File as CapFile, FileExt};
#[cfg(windows)]
use io_lifetimes::AsFilelike;
use system_interface::fs::Advice;
#[cfg(not(windows))]
use system_interface::fs::FileIoExt;

#[derive(Debug)]
pub(crate) struct CapFileWrapper<'a>(pub(crate) &'a CapFile);

impl<'a> From<&'a CapFile> for CapFileWrapper<'a> {
    fn from(v: &'a CapFile) -> Self {
        Self(v)
    }
}

impl CapFileWrapper<'_> {
    #[inline]
    pub(crate) fn read_at(&self, buf: &mut [u8], offset: u64) -> IoResult<usize> {
        loop {
            let r = cfg_select! {
                windows => FileExt::seek_read(self.0, buf, offset),
                _ => FileExt::read_at(self.0, buf, offset),
            };
            if let Err(e) = &r
                && e.kind() == ErrorKind::Interrupted
            {
                continue;
            }
            break r;
        }
    }

    #[inline]
    pub(crate) fn write_at(&self, buf: &[u8], offset: u64) -> IoResult<usize> {
        loop {
            let r = cfg_select! {
                windows => FileExt::seek_write(self.0, buf, offset),
                _ => FileExt::write_at(self.0, buf, offset),
            };
            if let Err(e) = &r
                && e.kind() == ErrorKind::Interrupted
            {
                continue;
            }
            break r;
        }
    }

    #[inline]
    pub(crate) fn append(&self, buf: &[u8]) -> IoResult<usize> {
        cfg_select! {
            windows => self.reopen_append()?.write(buf),
            _ => FileIoExt::append(self.0, buf),
        }
    }

    #[inline]
    pub(crate) fn advise(&self, offset: u64, len: u64, advice: Advice) -> IoResult<()> {
        cfg_select! {
            windows => {
                // TODO: Do something with the advice.
                let _ = (offset, len, advice);
                Ok(())
            }
            _ => FileIoExt::advise(self.0, offset, len, advice),
        }
    }

    #[cfg(windows)]
    fn reopen_append(&self) -> IoResult<File> {
        self.0
            .as_filelike_view::<File>()
            .reopen(OpenOptions::new().append(true))
    }
}

/*
impl IoExt for CapFileWrapper<'_> {
    fn read(&self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.read(buf)
    }

    fn read_exact(&self, buf: &mut [u8]) -> IoResult<()> {
        self.0.read_exact(buf)
    }

    fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> IoResult<usize> {
        self.0.read_vectored(bufs)
    }

    fn read_to_end(&self, buf: &mut Vec<u8>) -> IoResult<usize> {
        self.0.read_to_end(buf)
    }

    fn read_to_string(&self, buf: &mut String) -> IoResult<usize> {
        self.0.read_to_string(buf)
    }

    fn peek(&self, buf: &mut [u8]) -> IoResult<usize> {
        cfg_select! {
            windows => {
                use std::os::windows::io::AsRawHandle;

                let mut bytes_read = std::mem::MaybeUninit::<u32>::uninit();
                let len = std::cmp::min(buf.len(), u32::MAX as usize) as u32;
                let res = unsafe {
                    windows_sys::Win32::System::Pipes::PeekNamedPipe(
                        self.as_filelike().as_raw_handle() as _,
                        buf.as_mut_ptr() as *mut std::ffi::c_void,
                        len,
                        bytes_read.as_mut_ptr(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if res == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(unsafe { bytes_read.assume_init() } as usize)
            }
            _ => self.0.peek(buf),
        }
    }

    fn
write(&self, buf: &[u8]) -> IoResult<usize>;
    fn
write_all(&self, buf: &[u8]) -> IoResult<()>;
    fn
write_vectored(&self, bufs: &[IoSlice<'_>]) -> IoResult<usize>;
    fn
write_fmt(&self, fmt: Arguments<'_>) -> IoResult<()>;
    fn
flush(&self) -> IoResult<()>;
}

impl FileIoExt for CapFileWrapper<'_> {
    #[inline]
    fn advise(&self, offset: u64, len: u64, advice: Advice) -> IoResult<()> {
        cfg_select! {
            windows => {
                // TODO: Do something with the advice.
                let _ = (offset, len, advice);
                Ok(())
            }
            _ => self.0.advise(offset, len, advice),
        }
    }

    #[inline]
    fn allocate(&self, offset: u64, len: u64) -> IoResult<()> {
        cfg_select! {
            windows => {
                // We can't faithfully support allocate on Windows without exposing race
                // conditions. Instead, refuse:
                let _ = (offset, len);
                Err(IoError::new(
                    ErrorKind::PermissionDenied,
                    "file allocate is not supported on Windows",
                ))
            }
            _ => self.0.allocate(offset, len),
        }
    }

    #[inline]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> IoResult<usize> {
        cfg_select! {
            windows => self.0.seek_read(buf, offset),
            _ => self.0.read_at(buf, offset),
        }
    }

    #[inline]
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> IoResult<()> {
        cfg_select! {
            windows => {
                // Similar to `read_at`, re-open the file so that we can do a seek and
                // leave the original file unmodified.
                let reopened = loop {
                    match reopen(self) {
                        Ok(file) => break file,
                        Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                        Err(err) => return Err(err),
                    }
                };
                loop {
                    match reopened.seek(SeekFrom::Start(offset)) {
                        Ok(_) => break,
                        Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                        Err(err) => return Err(err),
                    }
                }
                reopened.read_exact(buf)
            }
            _ => self.0.read_exact_at(buf, offset),
        }
    }

    #[inline]
    fn read_vectored_at(&self, bufs: &mut [IoSliceMut], offset: u64) -> IoResult<usize> {
        cfg_select! {
            windows => {
                for buf in bufs {
                    if buf.is_empty() {
                        continue;
                    }
                    return self.0.seek_read(buf, offset);
                }
                Ok(0)
            }
            _ => self.0.read_vectored_at(bufs, offset),
        }
    }

    #[inline]
    fn read_exact_vectored_at(&self, bufs: &mut [IoSliceMut], offset: u64) -> IoResult<()> {
        cfg_select! {
            windows => {
                // Similar to `read_vectored_at`, re-open the file so that we can do a seek and
                // leave the original file unmodified.
                let reopened = loop {
                    match reopen(self) {
                        Ok(file) => break file,
                        Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                        Err(err) => return Err(err),
                    }
                };
                loop {
                    match reopened.seek(SeekFrom::Start(offset)) {
                        Ok(_) => break,
                        Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                        Err(err) => return Err(err),
                    }
                }
                reopened.read_exact_vectored(bufs)
            }
            _ => self.0.read_exact_vectored_at(bufs, offset),
        }
    }

    #[inline]
    fn is_read_vectored_at(&self) -> bool {
        cfg_select! {
            windows => false,
            _ => self.0.is_read_vectored_at(),
        }
    }

    #[inline]
    fn read_to_end_at(&self, buf: &mut Vec<u8>, offset: u64) -> IoResult<usize> {
        cfg_select! {
            windows => self.0.as_filelike_view::<File>().read_to_end_at(buf, offset),
            _ => self.0.read_to_end_at(buf, offset),
        }
    }

    #[inline]
    fn read_to_string_at(&self, buf: &mut String, offset: u64) -> IoResult<usize> {
        cfg_select! {
            windows => self.as_filelike_view::<File>().read_to_string_at(buf, offset),
            _ => self.0.read_to_string_at(buf, offset),
        }
    }

    #[inline]
    fn write_at(&self, buf: &[u8], offset: u64) -> IoResult<usize> {
        cfg_select! {
            windows => self.0.seek_write(buf, offset),
            _ => self.0.write_at(buf, offset),
        }
    }

    #[inline]
    fn write_all_at(&self, mut buf: &[u8], mut offset: u64) -> IoResult<()> {
        cfg_select! {
            windows => {
                // Similar to `read_exact_at`, re-open the file so that we can do a seek
                // and leave the original file unmodified.
                let reopened = reopen_write(self)?;
                while buf.len() > 0 {
                    let n = reopened.seek_write(buf, offset)?;
                    offset += u64::try_from(n).unwrap();
                    buf = &buf[n..];
                }
                Ok(())
            }
            _ => self.0.write_all_at(buf, offset),
        }
    }

    #[inline]
    fn write_vectored_at(&self, bufs: &[IoSlice], offset: u64) -> IoResult<usize> {
        cfg_select! {
            windows => {
                // Windows doesn't have a vectored write for files, so pick the first
                // non-empty slice and write that.
                for buf in bufs {
                    if buf.is_empty() {
                        continue;
                    }
                    return self.seek_write(buf, offset);
                }
                Ok(0)
            }
            _ => self.0.write_vectored(bufs, offset),
        }
    }

    #[inline]
    fn write_all_vectored_at(&self, bufs: &mut [IoSlice], mut offset: u64) -> IoResult<()> {
        cfg_select! {
            windows => {
                let reopened = reopen_write(self)?;
                for buf in bufs {
                    let mut buf = &buf[..];
                    while !buf.is_empty() {
                        let n = self.seek_write(buf, offset)?;
                        offset += u64::try_from(n).unwrap();
                        buf = &buf[n..];
                    }
                }
                Ok(())
            }
            _ => self.0.write_all_vectored_at(bufs, offset),
        }
    }

    #[inline]
    fn is_write_vectored_at(&self) -> bool {
        cfg_select! {
            windows => false,
            _ => self.0.is_write_vectored_at(),
        }
    }

    fn append(&self, buf: &[u8]) -> IoResult<usize> {
        // Re-open the file for appending.
        let reopened = reopen_append(self)?;
        reopened.write(buf)
    }

    fn append_vectored(&self, bufs: &[IoSlice]) -> IoResult<usize> {
        // Re-open the file for appending.
        let reopened = reopen_append(self)?;
        reopened.write_vectored(bufs)
    }

    #[inline]
    fn is_append_vectored(&self) -> bool {
        cfg_select! {
            windows => true,
            _ => self.0.is_append_vectored(),
        }
    }

    #[inline]
    fn seek(&self, pos: SeekFrom) -> IoResult<u64> {
        let mut p = self.0;
        p.seek(pos)
    }

    #[inline]
    fn stream_position(&self) -> IoResult<u64> {
        // This may eventually be obsoleted by [rust-lang/rust#59359].
        // [rust-lang/rust#59359]: https://github.com/rust-lang/rust/issues/59359.
        let mut p = self.0;
        p.seek(SeekFrom::Current(0))
    }
}

fn reopen(filelike: &CapFileWrapper) -> IoResult<File> {
    filelike
        .as_filelike_view::<File>()
        .reopen(cap_fs_ext::OpenOptions::new().read(true))
}

fn reopen_write(filelike: &CapFileWrapper) -> IoResult<File> {
    filelike
        .as_filelike_view::<File>()
        .reopen(cap_fs_ext::OpenOptions::new().write(true))
}
*/
