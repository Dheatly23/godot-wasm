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
