#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::{Error as IoError, Write};
use std::io::{ErrorKind, Result as IoResult};
#[cfg(windows)]
use std::mem::{MaybeUninit, size_of_val, zeroed};

#[cfg(windows)]
use cap_fs_ext::OpenOptions;
#[cfg(windows)]
use cap_fs_ext::Reopen;
use cap_std::fs::{File as CapFile, FileExt};
use io_lifetimes::AsFilelike;
#[cfg(not(windows))]
use rustix::{fs, io};

#[derive(Copy, Clone, Debug)]
pub(crate) enum Advice {
    Normal,
    Sequential,
    Random,
    WillNeed,
    DontNeed,
    NoReuse,
}

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
            // Yoink from wasmtime.
            _ => {
                // On Linux, use `pwritev2`.
                #[cfg(target_os = "linux")]
                {
                    let iovs = [std::io::IoSlice::new(buf)];
                    let ret = io::pwritev2(self.0, &iovs, 0, io::ReadWriteFlags::APPEND);
                    if !matches!(ret, Err(io::Errno::NOSYS) | Err(io::Errno::NOTSUP)) {
                        return ret.map_err(<_>::from);
                    }
                }

                // Otherwise use `F_SETFL` to switch the file description to append
                // mode, do the write, and switch back. This is not atomic with
                // respect to other users of the file description, but WASI isn't fully
                // threaded right now anyway.
                let old_flags = fs::fcntl_getfl(self.0)?;
                fs::fcntl_setfl(self.0, old_flags | fs::OFlags::APPEND)?;
                let ret = io::write(self.0, buf);
                fs::fcntl_setfl(self.0, old_flags).unwrap();
                ret.map_err(<_>::from)
            }
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
            // Yoink from wasmtime.
            target_vendor = "apple" => match advice {
                Advice::WillNeed => fs::fcntl_rdadvise(self.0, offset, len).map_err(<_>::from),
                Advice::Normal
                | Advice::Sequential
                | Advice::Random
                | Advice::DontNeed
                | Advice::NoReuse => Ok(()),
            },
            any(target_os = "linux", target_os = "android") => {
                let advice = match advice {
                    Advice::Normal => fs::Advice::Normal,
                    Advice::Sequential => fs::Advice::Sequential,
                    Advice::Random => fs::Advice::Random,
                    Advice::WillNeed => fs::Advice::WillNeed,
                    Advice::DontNeed => fs::Advice::DontNeed,
                    Advice::NoReuse => fs::Advice::NoReuse,
                };
                fs::fadvise(self.0, offset, std::num::NonZeroU64::new(len), advice)
                    .map_err(<_>::from)
            }
            _ => {
                // noop on other platforms
                let _ = (self.0, offset, len, advice);
                Ok(())
            }
        }
    }

    #[cfg(windows)]
    fn reopen_append(&self) -> IoResult<File> {
        self.0
            .as_filelike_view::<File>()
            .reopen(OpenOptions::new().append(true))
    }
}

bitflags::bitflags! {
    pub(crate) struct FdFlags: u32 {
        const DSYNC = 1 << 0;
        const RSYNC = 1 << 1;
        const SYNC = 1 << 2;
    }
}

#[allow(dropping_copy_types)]
pub(crate) fn get_fdflags(f: &impl AsFilelike) -> IoResult<FdFlags> {
    let mut ret = FdFlags::empty();

    cfg_select! {
        windows => {
            use std::os::windows::io::AsRawHandle;

            let mode = unsafe {
                let mut io_status_block =
                    zeroed::<windows_sys::Win32::System::IO::IO_STATUS_BLOCK>();
                let mut payload = MaybeUninit::<
                    windows_sys::Wdk::Storage::FileSystem::FILE_MODE_INFORMATION,
                >::uninit();

                let h = f.as_filelike();
                let status = windows_sys::Wdk::Storage::FileSystem::NtQueryInformationFile(
                    h.as_raw_handle(),
                    &mut io_status_block,
                    payload.as_mut_ptr().cast(),
                    size_of_val(&payload) as _,
                    windows_sys::Wdk::Storage::FileSystem::FileModeInformation,
                );
                drop(h);

                if status != windows_sys::Win32::Foundation::STATUS_SUCCESS {
                    return Err(IoError::from_raw_os_error(
                        windows_sys::Win32::Foundation::RtlNtStatusToDosError(status) as i32,
                    ));
                }

                payload.assume_init().Mode
            };

            ret.set(
                FdFlags::DSYNC,
                mode & windows_sys::Wdk::Storage::FileSystem::FILE_WRITE_THROUGH != 0,
            );
        }
        _ => {
            let h = f.as_filelike();
            let flags = fs::fcntl_getfl(h)?;
            drop(h);
            #[cfg(not(target_os = "redox"))]
            ret.set(FdFlags::DSYNC, flags.contains(fs::OFlags::DSYNC));
            ret.set(FdFlags::SYNC, flags.contains(fs::OFlags::SYNC));
            #[cfg(not(any(target_vendor = "apple", target_os = "freebsd", target_os = "redox")))]
            ret.set(FdFlags::RSYNC, flags.contains(fs::OFlags::RSYNC));
        }
    }

    Ok(ret)
}
