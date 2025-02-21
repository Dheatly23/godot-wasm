use std::io::{Error as IoError, ErrorKind};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Error as AnyError, Result as AnyResult};
use camino::Utf8PathBuf;
use cap_fs_ext::{
    DirExt, FileTypeExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt, OpenOptionsMaybeDirExt,
    SystemTimeSpec as CapSystemTimeSpec,
};
use cap_std::fs::{Dir as CapDir, FileType, Metadata, OpenOptions};
use fs_set_times::{SetTimes, SystemTimeSpec};
use rand::prelude::*;
use system_interface::fs::{Advice, FdFlags, FileIoExt, GetSetFdFlags};
use tracing::{Level, instrument};
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::context::{Stdin, WasiContext, try_iso_fs};
use crate::fs_host::{CapWrapper as HostCapWrapper, Descriptor};
use crate::fs_isolated::{AccessMode, CreateParams, OpenMode};
use crate::items::Item;
use crate::poll::PollController;
use crate::stdio::NullStdio;
use crate::{EMPTY_BUF, NullPollable, errors, items};

impl wasi::io::poll::HostPollable for WasiContext {
    #[instrument(skip(self), err)]
    fn ready(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<bool> {
        Ok(match self.items.get_item(res)? {
            items::Poll::NullPoll(_) => true,
            items::Poll::StdinPoll(v) => v.is_ready(),
            items::Poll::ClockPoll(v) => v.is_ready(),
        })
    }

    #[instrument(skip(self), err)]
    fn block(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<()> {
        match self.items.get_item(res)? {
            items::Poll::NullPoll(_) => (),
            items::Poll::StdinPoll(v) => v.block(self.timeout)?,
            items::Poll::ClockPoll(v) => v.block(self.timeout)?,
        }
        Ok(())
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::io::poll::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn poll(&mut self, res: Vec<Resource<wasi::io::poll::Pollable>>) -> AnyResult<Vec<u32>> {
        let polls = self.items.get_item(res)?;
        match &*polls {
            [] => return Ok(Vec::new()),
            [v] => {
                match v {
                    items::Poll::NullPoll(_) => (),
                    items::Poll::StdinPoll(v) => v.block(self.timeout)?,
                    items::Poll::ClockPoll(v) => v.block(self.timeout)?,
                }
                return Ok(vec![0]);
            }
            _ => (),
        }

        let mut controller: Option<PollController> = None;
        for _ in 0..3 {
            let ret: Vec<_> = polls
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if match p {
                        items::Poll::NullPoll(_) => true,
                        items::Poll::StdinPoll(v) => {
                            controller.as_ref().is_some_and(|c| c.is_waited(&v.0)) || v.is_ready()
                        }
                        items::Poll::ClockPoll(v) => v.is_ready(),
                    } {
                        Some(i as u32)
                    } else {
                        None
                    }
                })
                .collect();
            if !ret.is_empty() {
                return Ok(ret);
            }

            let c = controller.get_or_insert_with(|| {
                let mut c = PollController::new(self.timeout);
                for i in &polls {
                    match i {
                        items::Poll::NullPoll(_) => (),
                        items::Poll::StdinPoll(v) => c.add_signal(&v.0),
                        items::Poll::ClockPoll(v) => c.set_instant(v.until),
                    }
                }

                c
            });
            if c.poll() {
                break;
            }
        }

        Ok(Vec::new())
    }
}

impl wasi::io::error::HostError for WasiContext {
    #[instrument(skip(self), ret, err)]
    fn to_debug_string(&mut self, res: Resource<wasi::io::error::Error>) -> AnyResult<String> {
        // No way to construct stream error
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::io::error::Error>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::io::error::Host for WasiContext {}

impl wasi::io::streams::HostInputStream for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn read(
        &mut self,
        res: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<Vec<u8>, errors::StreamError> {
        let len = len.try_into().unwrap_or(usize::MAX);
        Ok(match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => Vec::new(),
            items::IOStream::IsoFSAccess(mut v) => v.read(len)?,
            items::IOStream::HostFSStream(mut v) => v.read(len)?,
            items::IOStream::StdinSignal(v) => v.read(len)?,
            items::IOStream::HostStdin(v) => v.read(len)?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn blocking_read(
        &mut self,
        res: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<Vec<u8>, errors::StreamError> {
        let len = len.try_into().unwrap_or(usize::MAX);
        Ok(match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => Vec::new(),
            items::IOStream::IsoFSAccess(mut v) => v.read(len)?,
            items::IOStream::HostFSStream(mut v) => v.read(len)?,
            items::IOStream::StdinSignal(v) => v.read_block(len, self.timeout)?,
            items::IOStream::HostStdin(v) => v.read_block(len, self.timeout)?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn skip(
        &mut self,
        res: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<u64, errors::StreamError> {
        let len = len.try_into().unwrap_or(usize::MAX);
        Ok(match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => 0,
            items::IOStream::IsoFSAccess(mut v) => v.skip(len)? as u64,
            items::IOStream::HostFSStream(mut v) => v.skip(len)? as u64,
            items::IOStream::StdinSignal(v) => v.skip(len)? as u64,
            items::IOStream::HostStdin(v) => v.skip(len)? as u64,
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn blocking_skip(
        &mut self,
        res: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<u64, errors::StreamError> {
        let len = len.try_into().unwrap_or(usize::MAX);
        Ok(match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => 0,
            items::IOStream::IsoFSAccess(mut v) => v.skip(len)? as u64,
            items::IOStream::HostFSStream(mut v) => v.skip(len)? as u64,
            items::IOStream::StdinSignal(v) => v.skip_block(len, self.timeout)? as u64,
            items::IOStream::HostStdin(v) => v.skip_block(len, self.timeout)? as u64,
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::io::streams::InputStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        let ret: Item = match self.items.get_item(res)? {
            items::IOStream::IsoFSAccess(v) => v.poll()?.into(),
            items::IOStream::StdinSignal(v) => v.poll()?.into(),
            items::IOStream::HostFSStream(_)
            | items::IOStream::HostStdin(_)
            | items::IOStream::NullStdio(_) => NullPollable::new().into(),
            _ => return Err(IoError::from(ErrorKind::InvalidInput).into()),
        };
        self.register(ret)
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::io::streams::InputStream>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::io::streams::HostOutputStream for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn check_write(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<u64, errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_)
            | items::IOStream::IsoFSAccess(_)
            | items::IOStream::HostFSStream(_)
            | items::IOStream::HostStdout(_) => Ok(65536),
            _ => Err(ErrorKind::InvalidInput.into()),
        }
    }

    #[instrument(skip(self, data), fields(data.len = data.len()), err(level = Level::WARN))]
    fn write(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        data: Vec<u8>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => (),
            items::IOStream::IsoFSAccess(mut v) => v.write(&data)?,
            items::IOStream::HostFSStream(mut v) => v.write(&data)?,
            items::IOStream::HostStdout(v) => v.write(&data)?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    #[instrument(skip(self, data), fields(data.len = data.len()), err(level = Level::WARN))]
    fn blocking_write_and_flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        data: Vec<u8>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => (),
            items::IOStream::IsoFSAccess(mut v) => v.write(&data)?,
            items::IOStream::HostFSStream(mut v) => v.write(&data)?,
            items::IOStream::HostStdout(v) => {
                v.write(&data)?;
                v.flush()?;
            }
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<(), errors::StreamError> {
        self.blocking_flush(res)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn blocking_flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_)
            | items::IOStream::IsoFSAccess(_)
            | items::IOStream::HostFSStream(_) => (),
            items::IOStream::HostStdout(v) => v.flush()?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        let ret: Item = match self.items.get_item(res)? {
            items::IOStream::IsoFSAccess(v) => v.poll()?.into(),
            items::IOStream::NullStdio(_)
            | items::IOStream::HostFSStream(_)
            | items::IOStream::HostStdout(_) => NullPollable::new().into(),
            _ => return Err(IoError::from(ErrorKind::InvalidInput).into()),
        };
        self.register(ret)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn write_zeroes(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        mut len: u64,
    ) -> Result<(), errors::StreamError> {
        let mut v = self.items.get_item(res)?;
        while len > 0 {
            let data = &EMPTY_BUF[..len.min(EMPTY_BUF.len() as u64) as usize];
            match &mut v {
                items::IOStream::NullStdio(_) => (),
                items::IOStream::IsoFSAccess(v) => v.write(data)?,
                items::IOStream::HostFSStream(v) => v.write(data)?,
                items::IOStream::HostStdout(v) => v.write(data)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
            len -= data.len() as u64;
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn blocking_write_zeroes_and_flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        mut len: u64,
    ) -> Result<(), errors::StreamError> {
        let mut v = self.items.get_item(res)?;
        while len > 0 {
            let data = &EMPTY_BUF[..len.min(EMPTY_BUF.len() as u64) as usize];
            match &mut v {
                items::IOStream::NullStdio(_) => (),
                items::IOStream::IsoFSAccess(v) => v.write(data)?,
                items::IOStream::HostFSStream(v) => v.write(data)?,
                items::IOStream::HostStdout(v) => v.write(data)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
            len -= data.len() as u64;
        }

        match v {
            items::IOStream::NullStdio(_)
            | items::IOStream::IsoFSAccess(_)
            | items::IOStream::HostFSStream(_) => (),
            items::IOStream::HostStdout(v) => v.flush()?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn splice(
        &mut self,
        output: Resource<wasi::io::streams::OutputStream>,
        input: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<u64, errors::StreamError> {
        let (mut input, mut output) = self.items.get_item((input, output))?;
        match (&input, &output) {
            (items::IOStream::NullStdio(_), _) | (_, items::IOStream::NullStdio(_)) => {
                return Ok(0);
            }
            (
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::HostFSStream(_)
                | items::IOStream::StdinSignal(_)
                | items::IOStream::HostStdin(_),
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::HostFSStream(_)
                | items::IOStream::HostStdout(_),
            ) => (),
            _ => return Err(ErrorKind::InvalidInput.into()),
        }

        let mut n = 0;
        let mut l = usize::try_from(len).unwrap_or(usize::MAX);
        while l > 0 {
            let i = l.min(4096);

            let b = match &mut input {
                items::IOStream::IsoFSAccess(v) => v.read(i)?,
                items::IOStream::HostFSStream(v) => v.read(i)?,
                items::IOStream::StdinSignal(v) => v.read(i)?,
                items::IOStream::HostStdin(v) => v.read(i)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            };
            if b.is_empty() {
                break;
            }
            l -= b.len();
            n += b.len();

            match &mut output {
                items::IOStream::IsoFSAccess(v) => v.write(&b)?,
                items::IOStream::HostStdout(v) => v.write(&b)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
        }

        Ok(n as u64)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn blocking_splice(
        &mut self,
        output: Resource<wasi::io::streams::OutputStream>,
        input: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<u64, errors::StreamError> {
        let (mut input, mut output) = self.items.get_item((input, output))?;
        match (&input, &output) {
            (items::IOStream::NullStdio(_), _) | (_, items::IOStream::NullStdio(_)) => {
                return Ok(0);
            }
            (
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::StdinSignal(_)
                | items::IOStream::HostStdin(_),
                items::IOStream::IsoFSAccess(_) | items::IOStream::HostStdout(_),
            ) => (),
            _ => return Err(ErrorKind::InvalidInput.into()),
        }

        let mut n = 0;
        let mut l = usize::try_from(len).unwrap_or(usize::MAX);
        while l > 0 {
            let i = l.min(4096);

            let b = match &mut input {
                items::IOStream::IsoFSAccess(v) => v.read(i)?,
                items::IOStream::StdinSignal(v) => v.read_block(i, self.timeout)?,
                items::IOStream::HostStdin(v) => v.read_block(i, self.timeout)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            };
            if b.is_empty() {
                break;
            }
            l -= b.len();
            n += b.len();

            match &mut output {
                items::IOStream::IsoFSAccess(v) => v.write(&b)?,
                items::IOStream::HostFSStream(v) => v.write(&b)?,
                items::IOStream::HostStdout(v) => v.write(&b)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
        }

        Ok(n as u64)
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::io::streams::OutputStream>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::io::streams::Host for WasiContext {
    #[instrument(level = Level::ERROR, skip_all, fields(%e), ret, err)]
    fn convert_stream_error(
        &mut self,
        e: errors::StreamError,
    ) -> AnyResult<wasi::io::streams::StreamError> {
        e.into()
    }
}

fn set_time(time: wasi::filesystem::types::NewTimestamp, now: &SystemTime, dst: &mut SystemTime) {
    match time {
        wasi::filesystem::types::NewTimestamp::NoChange => (),
        wasi::filesystem::types::NewTimestamp::Now => *dst = *now,
        wasi::filesystem::types::NewTimestamp::Timestamp(t) => {
            *dst = SystemTime::UNIX_EPOCH + Duration::new(t.seconds, t.nanoseconds)
        }
    }
}

fn time_cvt(time: wasi::filesystem::types::NewTimestamp) -> Option<SystemTimeSpec> {
    match time {
        wasi::filesystem::types::NewTimestamp::NoChange => None,
        wasi::filesystem::types::NewTimestamp::Now => Some(SystemTimeSpec::SymbolicNow),
        wasi::filesystem::types::NewTimestamp::Timestamp(v) => Some(SystemTimeSpec::Absolute(
            SystemTime::UNIX_EPOCH + Duration::new(v.seconds, v.nanoseconds),
        )),
    }
}

fn desc_type(f: FileType) -> wasi::filesystem::types::DescriptorType {
    if f.is_dir() {
        wasi::filesystem::types::DescriptorType::Directory
    } else if f.is_symlink() {
        wasi::filesystem::types::DescriptorType::SymbolicLink
    } else if f.is_block_device() {
        wasi::filesystem::types::DescriptorType::BlockDevice
    } else if f.is_char_device() {
        wasi::filesystem::types::DescriptorType::CharacterDevice
    } else if f.is_file() {
        wasi::filesystem::types::DescriptorType::RegularFile
    } else {
        wasi::filesystem::types::DescriptorType::Unknown
    }
}

fn meta_to_stat(m: Metadata) -> wasi::filesystem::types::DescriptorStat {
    fn to_datetime(t: cap_std::time::SystemTime) -> Option<wasi::clocks::wall_clock::Datetime> {
        let t = t.into_std().duration_since(SystemTime::UNIX_EPOCH).ok()?;
        Some(wasi::clocks::wall_clock::Datetime {
            seconds: t.as_secs(),
            nanoseconds: t.subsec_nanos(),
        })
    }

    wasi::filesystem::types::DescriptorStat {
        type_: desc_type(m.file_type()),
        link_count: m.nlink(),
        size: m.len(),
        data_access_timestamp: m.accessed().ok().and_then(to_datetime),
        data_modification_timestamp: m.modified().ok().and_then(to_datetime),
        status_change_timestamp: m.created().ok().and_then(to_datetime),
    }
}

impl WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn open_file<T: 'static>(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        mode: OpenMode,
    ) -> Result<Resource<T>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => Box::new(v.open_file(mode)?).into(),
            items::Desc::HostFSDesc(v) => Box::new(v.open_file(mode)?).into(),
        };
        Ok(self.register(ret)?)
    }
}

impl wasi::filesystem::types::HostDescriptor for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn read_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<Resource<wasi::io::streams::InputStream>, errors::StreamError> {
        self.open_file(res, OpenMode::Read(off.try_into()?))
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn write_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<Resource<wasi::io::streams::OutputStream>, errors::StreamError> {
        self.open_file(res, OpenMode::Write(off.try_into()?))
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn append_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<Resource<wasi::io::streams::OutputStream>, errors::StreamError> {
        self.open_file(res, OpenMode::Append)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn advise(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
        len: wasi::filesystem::types::Filesize,
        advice: wasi::filesystem::types::Advice,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) => (),
            items::Desc::HostFSDesc(v) => v.file()?.advise(
                off,
                len,
                match advice {
                    wasi::filesystem::types::Advice::Normal => Advice::Normal,
                    wasi::filesystem::types::Advice::Sequential => Advice::Sequential,
                    wasi::filesystem::types::Advice::Random => Advice::Random,
                    wasi::filesystem::types::Advice::WillNeed => Advice::WillNeed,
                    wasi::filesystem::types::Advice::DontNeed => Advice::DontNeed,
                    wasi::filesystem::types::Advice::NoReuse => Advice::NoReuse,
                },
            )?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sync_data(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) => (),
            items::Desc::HostFSDesc(v) => v.sync_data()?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn get_flags(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::DescriptorFlags, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.file_flags(),
            items::Desc::HostFSDesc(v) => {
                let f = match &**v.desc() {
                    Descriptor::File(v) => v.get_fd_flags(),
                    Descriptor::Dir(v) => v.get_fd_flags(),
                }?;

                let mut r = wasi::filesystem::types::DescriptorFlags::empty();
                if v.access().is_read() {
                    r |= wasi::filesystem::types::DescriptorFlags::READ;
                }
                if v.access().is_write() {
                    r |= match v.desc().dir() {
                        Some(_) => wasi::filesystem::types::DescriptorFlags::MUTATE_DIRECTORY,
                        None => wasi::filesystem::types::DescriptorFlags::WRITE,
                    };
                }
                if f.contains(FdFlags::DSYNC) {
                    r |= wasi::filesystem::types::DescriptorFlags::REQUESTED_WRITE_SYNC;
                }
                if f.contains(FdFlags::RSYNC) {
                    r |= wasi::filesystem::types::DescriptorFlags::DATA_INTEGRITY_SYNC;
                }
                if f.contains(FdFlags::SYNC) {
                    r |= wasi::filesystem::types::DescriptorFlags::FILE_INTEGRITY_SYNC;
                }

                Ok(r)
            }
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn get_type(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::DescriptorType, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.file_type(),
            items::Desc::HostFSDesc(v) => Ok(match &**v.desc() {
                Descriptor::Dir(_) => wasi::filesystem::types::DescriptorType::Directory,
                Descriptor::File(v) => desc_type(v.metadata()?.file_type()),
            }),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_size(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        size: wasi::filesystem::types::Filesize,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.resize(size.try_into()?)?,
            items::Desc::HostFSDesc(v) => v.write()?.file()?.set_len(size)?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_times(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        atime: wasi::filesystem::types::NewTimestamp,
        mtime: wasi::filesystem::types::NewTimestamp,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.set_time(|stamp| -> Result<_, errors::StreamError> {
                let now = SystemTime::now();
                set_time(mtime, &now, &mut stamp.mtime);
                set_time(atime, &now, &mut stamp.atime);
                Ok(())
            })?,
            items::Desc::HostFSDesc(v) => {
                let atime = time_cvt(atime);
                let mtime = time_cvt(mtime);
                match &**v.write()?.desc() {
                    Descriptor::File(v) => v.set_times(atime, mtime),
                    Descriptor::Dir(v) => SetTimes::set_times(v, atime, mtime),
                }?
            }
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn read(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        len: wasi::filesystem::types::Filesize,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<(Vec<u8>, bool), errors::StreamError> {
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        Ok(match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                if let Ok(off) = usize::try_from(off) {
                    let r = v.read(len, off)?;
                    let b = len != 0 && r.is_empty();
                    (r, b)
                } else {
                    (Vec::new(), true)
                }
            }
            items::Desc::HostFSDesc(v) => {
                let v = v.read()?.file()?;
                let mut ret = vec![0; len];
                let i = HostCapWrapper::read_at(v, &mut ret, off)?;
                if !ret.is_empty() && i == 0 {
                    (Vec::new(), true)
                } else {
                    ret.truncate(i);
                    (ret, false)
                }
            }
        })
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn write(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        buf: Vec<u8>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<wasi::filesystem::types::Filesize, errors::StreamError> {
        Ok(match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                v.write(&buf, off.try_into()?)?;
                buf.len() as _
            }
            items::Desc::HostFSDesc(v) => v.write()?.file()?.write_at(&buf, off)? as _,
        })
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn read_directory(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<Resource<wasi::filesystem::types::DirectoryEntryStream>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => Box::new(v.read_directory()?).into(),
            items::Desc::HostFSDesc(v) => Box::new(v.read_dir()?).into(),
        };
        Ok(self.register(ret)?)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sync(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) => (),
            items::Desc::HostFSDesc(v) => v.sync()?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn create_directory_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path: String,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                let p = Utf8PathBuf::from(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let controller = try_iso_fs(&self.iso_fs)?;

                v.open(controller, parent, true, None, AccessMode::W)?
                    .create_dir(controller, name)?;
            }
            items::Desc::HostFSDesc(v) => v.write()?.dir()?.create_dir(path)?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn stat(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::DescriptorStat, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.stat(),
            items::Desc::HostFSDesc(v) => Ok(meta_to_stat(match &**v.desc() {
                Descriptor::File(v) => v.metadata(),
                Descriptor::Dir(v) => v.dir_metadata(),
            }?)),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn stat_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path_flags: wasi::filesystem::types::PathFlags,
        path: String,
    ) -> Result<wasi::filesystem::types::DescriptorStat, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v
                .open(
                    try_iso_fs(&self.iso_fs)?,
                    &Utf8PathBuf::from(path),
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                    None,
                    AccessMode::RW,
                )?
                .stat(),
            items::Desc::HostFSDesc(v) => {
                let v = v.dir()?;
                Ok(meta_to_stat(if path_flags
                    .contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW)
                {
                    v.metadata(path)
                } else {
                    v.symlink_metadata(path)
                }?))
            }
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_times_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path_flags: wasi::filesystem::types::PathFlags,
        path: String,
        atime: wasi::filesystem::types::NewTimestamp,
        mtime: wasi::filesystem::types::NewTimestamp,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v
                .open(
                    try_iso_fs(&self.iso_fs)?,
                    &Utf8PathBuf::from(path),
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                    None,
                    AccessMode::W,
                )?
                .set_time(|stamp| -> Result<_, errors::StreamError> {
                    let now = SystemTime::now();
                    set_time(mtime, &now, &mut stamp.mtime);
                    set_time(atime, &now, &mut stamp.atime);
                    Ok(())
                })?,
            items::Desc::HostFSDesc(v) => {
                let v = v.write()?.dir()?;
                let atime = time_cvt(atime).map(CapSystemTimeSpec::from_std);
                let mtime = time_cvt(mtime).map(CapSystemTimeSpec::from_std);
                if path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW) {
                    DirExt::set_times(v, path, atime, mtime)
                } else {
                    v.set_symlink_times(path, atime, mtime)
                }?
            }
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn link_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        _flags: wasi::filesystem::types::PathFlags,
        _old_path: String,
        _new: Resource<wasi::filesystem::types::Descriptor>,
        _new_path: String,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) | items::Desc::HostFSDesc(_) => {
                Err(ErrorKind::Unsupported.into())
            }
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn open_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path_flags: wasi::filesystem::types::PathFlags,
        path: String,
        open_flags: wasi::filesystem::types::OpenFlags,
        flags: wasi::filesystem::types::DescriptorFlags,
    ) -> Result<Resource<wasi::filesystem::types::Descriptor>, errors::StreamError> {
        let symlink = path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW);
        let mut access = match (
            flags.contains(wasi::filesystem::types::DescriptorFlags::READ),
            flags.intersects(
                wasi::filesystem::types::DescriptorFlags::WRITE
                    | wasi::filesystem::types::DescriptorFlags::MUTATE_DIRECTORY,
            ),
        ) {
            (_, false) => AccessMode::R,
            (false, true) => AccessMode::W,
            (true, true) => AccessMode::RW,
        };
        let create = open_flags.contains(wasi::filesystem::types::OpenFlags::CREATE);
        let exclusive = open_flags.contains(wasi::filesystem::types::OpenFlags::EXCLUSIVE);
        let is_dir = open_flags.contains(wasi::filesystem::types::OpenFlags::DIRECTORY);
        let is_truncate = open_flags.contains(wasi::filesystem::types::OpenFlags::TRUNCATE);
        if is_dir && is_truncate {
            return Err(ErrorKind::InvalidInput.into());
        }

        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                let create = if create {
                    access = access | AccessMode::W;
                    Some(CreateParams {
                        dir: is_dir,
                        exclusive,
                    })
                } else {
                    None
                };

                let controller = try_iso_fs(&self.iso_fs)?;
                let v = v
                    .open(
                        controller,
                        &Utf8PathBuf::from(path),
                        symlink,
                        create,
                        access,
                    )?
                    .follow_symlink(controller)?;
                if is_dir && !v.node().is_dir() {
                    return Err(ErrorKind::NotADirectory.into());
                }
                if is_truncate {
                    v.resize(0)?;
                }

                Box::new(v).into()
            }
            items::Desc::HostFSDesc(v) => {
                access = access & v.access();
                let mut opts = OpenOptions::new();
                if create {
                    v.access().write_or_err()?;
                    access = access | AccessMode::W;
                    if exclusive {
                        opts.create_new(true);
                    } else {
                        opts.create(true);
                    }
                }
                access.access_or_err()?;
                match access {
                    AccessMode::NA | AccessMode::R => opts.read(true),
                    AccessMode::W => opts.write(true),
                    AccessMode::RW => opts.read(true).write(true),
                };
                if is_truncate {
                    opts.truncate(true);
                }
                opts.follow(if symlink {
                    FollowSymlinks::Yes
                } else {
                    FollowSymlinks::No
                });
                if is_dir {
                    opts.maybe_dir(true);
                }

                let v = v.dir()?.open_with(path, &opts)?;
                let v = if v.metadata()?.is_dir() {
                    Descriptor::Dir(CapDir::from_std_file(v.into_std()))
                } else if is_dir {
                    return Err(ErrorKind::NotADirectory.into());
                } else {
                    Descriptor::File(v)
                };
                Box::new(HostCapWrapper::new(Arc::new(v), access)).into()
            }
        };
        Ok(self.register(ret)?)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn readlink_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path: String,
    ) -> Result<String, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v
                .open(
                    try_iso_fs(&self.iso_fs)?,
                    &Utf8PathBuf::from(path),
                    false,
                    None,
                    AccessMode::R,
                )?
                .read_link(),
            items::Desc::HostFSDesc(v) => v
                .read()?
                .dir()?
                .read_link(path)?
                .into_os_string()
                .into_string()
                .map_err(|_| ErrorKind::InvalidData.into()),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn remove_directory_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path: String,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                let p = Utf8PathBuf::from(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };

                v.open(try_iso_fs(&self.iso_fs)?, parent, true, None, AccessMode::W)?
                    .unlink(name, true)?;
            }
            items::Desc::HostFSDesc(v) => v.write()?.dir()?.remove_dir(path)?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn rename_at(
        &mut self,
        src: Resource<wasi::filesystem::types::Descriptor>,
        src_path: String,
        dst: Resource<wasi::filesystem::types::Descriptor>,
        dst_path: String,
    ) -> Result<(), errors::StreamError> {
        let res = (src, dst);
        match self.items.get_item_ref(&res)? {
            (items::DescR::IsoFSNode(src), items::DescR::IsoFSNode(dst)) => {
                let (src_path, dst_path) =
                    (Utf8PathBuf::from(src_path), Utf8PathBuf::from(dst_path));
                let (src_path, Some(src_file), dst_path, Some(dst_file)) = (
                    src_path.parent().unwrap_or(&src_path),
                    src_path.file_name(),
                    dst_path.parent().unwrap_or(&dst_path),
                    dst_path.file_name(),
                ) else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let controller = try_iso_fs(&self.iso_fs)?;

                let src = src.open(controller, src_path, true, None, AccessMode::W)?;
                let dst = dst.open(controller, dst_path, true, None, AccessMode::W)?;

                dst.move_file(src.node(), src_file, dst_file)?;
            }
            (items::DescR::HostFSDesc(src), items::DescR::HostFSDesc(dst)) => {
                src.write()?
                    .dir()?
                    .rename(src_path, dst.write()?.dir()?, dst_path)?
            }
            _ => return Err(wasi::filesystem::types::ErrorCode::CrossDevice.into()),
        }
        self.items.maybe_unregister(res);
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn symlink_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path: String,
        target: String,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                let p = Utf8PathBuf::from(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let controller = try_iso_fs(&self.iso_fs)?;

                v.open(controller, parent, true, None, AccessMode::W)?
                    .create_link(controller, name, &Utf8PathBuf::from(target))?;
            }
            // Universally unsupport symlink
            items::Desc::HostFSDesc(_) => return Err(ErrorKind::Unsupported.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn unlink_file_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path: String,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                let p = Utf8PathBuf::from(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };

                v.open(try_iso_fs(&self.iso_fs)?, parent, true, None, AccessMode::W)?
                    .unlink(name, false)?;
            }
            items::Desc::HostFSDesc(v) => v.write()?.dir()?.remove_file_or_symlink(path)?,
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn is_same_object(
        &mut self,
        a: Resource<wasi::filesystem::types::Descriptor>,
        b: Resource<wasi::filesystem::types::Descriptor>,
    ) -> AnyResult<bool> {
        let res = (a, b);
        let ret = match self.items.get_item_ref(&res)? {
            (items::DescR::IsoFSNode(a), items::DescR::IsoFSNode(b)) => a.is_same(b),
            (items::DescR::HostFSDesc(a), items::DescR::HostFSDesc(b)) => {
                // Mostly copy upstream
                let meta_a = match &**a.desc() {
                    Descriptor::File(v) => v.metadata(),
                    Descriptor::Dir(v) => v.dir_metadata(),
                }?;
                let meta_b = match &**b.desc() {
                    Descriptor::File(v) => v.metadata(),
                    Descriptor::Dir(v) => v.dir_metadata(),
                }?;
                meta_a.dev() == meta_b.dev() && meta_a.ino() == meta_b.ino()
            }
            _ => false,
        };
        self.items.maybe_unregister(res);
        Ok(ret)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn metadata_hash(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::MetadataHashValue, errors::StreamError> {
        match self.items.get_item_ref(&res)? {
            items::DescR::IsoFSNode(v) => Ok(v.metadata_hash(&self.hasher)),
            items::DescR::HostFSDesc(v) => v.metadata_hash(&self.hasher),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn metadata_hash_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path_flags: wasi::filesystem::types::PathFlags,
        path: String,
    ) -> Result<wasi::filesystem::types::MetadataHashValue, errors::StreamError> {
        match self.items.get_item_ref(&res)? {
            items::DescR::IsoFSNode(v) => Ok(v
                .open(
                    try_iso_fs(&self.iso_fs)?,
                    &Utf8PathBuf::from(path),
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                    None,
                    AccessMode::RW,
                )?
                .metadata_hash(&self.hasher)),
            items::DescR::HostFSDesc(v) => v.metadata_hash_at(
                path,
                path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                &self.hasher,
            ),
        }
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::filesystem::types::Descriptor>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::filesystem::types::HostDirectoryEntryStream for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn read_directory_entry(
        &mut self,
        res: Resource<wasi::filesystem::types::DirectoryEntryStream>,
    ) -> Result<Option<wasi::filesystem::types::DirectoryEntry>, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Readdir::IsoFSReaddir(mut v) => v.next().map(|v| {
                v.map(|(k, v)| wasi::filesystem::types::DirectoryEntry {
                    name: k.to_string(),
                    type_: v.file_type(),
                })
            }),
            items::Readdir::HostFSReaddir(v) => (&**v).next().transpose()?.map(|v| {
                Ok(wasi::filesystem::types::DirectoryEntry {
                    type_: desc_type(v.metadata()?.file_type()),
                    name: v
                        .file_name()
                        .into_string()
                        .map_err(|_| ErrorKind::InvalidData)?,
                })
            }),
        }
        .transpose()
    }

    #[instrument(skip(self), err)]
    fn drop(
        &mut self,
        res: Resource<wasi::filesystem::types::DirectoryEntryStream>,
    ) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::filesystem::types::Host for WasiContext {
    #[instrument(level = Level::WARN, skip(self), err)]
    fn filesystem_error_code(
        &mut self,
        res: Resource<wasi::filesystem::types::Error>,
    ) -> AnyResult<Option<wasi::filesystem::types::ErrorCode>> {
        // No way to construct stream error
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(level = Level::DEBUG, skip(self), err)]
    fn convert_error_code(
        &mut self,
        e: errors::StreamError,
    ) -> AnyResult<wasi::filesystem::types::ErrorCode> {
        e.into()
    }
}

impl wasi::filesystem::preopens::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_directories(
        &mut self,
    ) -> AnyResult<Vec<(Resource<wasi::filesystem::preopens::Descriptor>, String)>> {
        self.preopens
            .iter()
            .map(|(p, v)| {
                let i = self.items.insert(v.into());
                match u32::try_from(i) {
                    Ok(i) => Ok((Resource::new_own(i), p.to_string())),
                    Err(e) => {
                        self.items.remove(i);
                        Err(AnyError::from(e))
                    }
                }
            })
            .collect()
    }
}

impl wasi::clocks::monotonic_clock::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn now(&mut self) -> AnyResult<wasi::clocks::monotonic_clock::Instant> {
        Ok(self.clock.now())
    }

    #[instrument(skip(self), err)]
    fn resolution(&mut self) -> AnyResult<wasi::clocks::monotonic_clock::Duration> {
        Ok(1000)
    }

    #[instrument(skip(self), err)]
    fn subscribe_instant(
        &mut self,
        when: wasi::clocks::monotonic_clock::Instant,
    ) -> AnyResult<Resource<wasi::clocks::monotonic_clock::Pollable>> {
        let ret = Item::from(Box::new(self.clock.poll_until(when)?));
        self.register(ret)
    }

    #[instrument(skip(self), err)]
    fn subscribe_duration(
        &mut self,
        when: wasi::clocks::monotonic_clock::Duration,
    ) -> AnyResult<Resource<wasi::clocks::monotonic_clock::Pollable>> {
        let ret = Item::from(Box::new(self.clock.poll_for(when)?));
        self.register(ret)
    }
}

impl wasi::clocks::wall_clock::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn now(&mut self) -> AnyResult<wasi::clocks::wall_clock::Datetime> {
        let t = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        Ok(wasi::clocks::wall_clock::Datetime {
            seconds: t.as_secs(),
            nanoseconds: t.subsec_nanos(),
        })
    }

    #[instrument(skip(self), err)]
    fn resolution(&mut self) -> AnyResult<wasi::clocks::wall_clock::Datetime> {
        Ok(wasi::clocks::wall_clock::Datetime {
            seconds: 0,
            nanoseconds: 1000,
        })
    }
}

impl wasi::clocks::timezone::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn display(
        &mut self,
        time: wasi::clocks::timezone::Datetime,
    ) -> AnyResult<wasi::clocks::timezone::TimezoneDisplay> {
        self.clock_tz.display(time)
    }

    #[instrument(skip(self), err)]
    fn utc_offset(&mut self, time: wasi::clocks::timezone::Datetime) -> AnyResult<i32> {
        self.clock_tz.utc_offset(time)
    }
}

impl wasi::random::insecure::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_insecure_random_bytes(&mut self, len: u64) -> AnyResult<Vec<u8>> {
        let mut ret = vec![0u8; len.try_into()?];
        self.insecure_rng.fill(&mut ret[..]);
        Ok(ret)
    }

    #[instrument(skip(self), err)]
    fn get_insecure_random_u64(&mut self) -> AnyResult<u64> {
        Ok(self.insecure_rng.random())
    }
}

impl wasi::random::insecure_seed::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn insecure_seed(&mut self) -> AnyResult<(u64, u64)> {
        Ok(self.insecure_rng.random())
    }
}

impl wasi::random::random::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_random_bytes(&mut self, len: u64) -> AnyResult<Vec<u8>> {
        let mut ret = vec![0u8; len.try_into()?];
        self.secure_rng.fill(&mut ret[..]);
        Ok(ret)
    }

    #[instrument(skip(self), err)]
    fn get_random_u64(&mut self) -> AnyResult<u64> {
        Ok(self.secure_rng.random())
    }
}

impl wasi::sockets::network::HostNetwork for WasiContext {
    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::sockets::network::Network>) -> AnyResult<()> {
        // No way to construct network connection
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::network::Host for WasiContext {
    #[instrument(level = Level::WARN, skip(self), err)]
    fn network_error_code(
        &mut self,
        res: Resource<wasi::sockets::network::Error>,
    ) -> AnyResult<Option<wasi::sockets::network::ErrorCode>> {
        // No way to construct network error
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(level = Level::DEBUG, skip(self), err)]
    fn convert_error_code(
        &mut self,
        e: errors::NetworkError,
    ) -> AnyResult<wasi::sockets::network::ErrorCode> {
        e.into()
    }
}

impl wasi::sockets::instance_network::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn instance_network(&mut self) -> AnyResult<Resource<wasi::sockets::network::Network>> {
        Err(errors::NetworkUnsupportedError.into())
    }
}

impl wasi::sockets::ip_name_lookup::HostResolveAddressStream for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn resolve_next_address(
        &mut self,
        res: Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>,
    ) -> Result<Option<wasi::sockets::network::IpAddress>, errors::NetworkError> {
        // No way to construct resolve address
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err)]
    fn drop(
        &mut self,
        res: Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>,
    ) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::ip_name_lookup::Host for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn resolve_addresses(
        &mut self,
        res: Resource<wasi::sockets::network::Network>,
        _name: String,
    ) -> Result<Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>, errors::NetworkError>
    {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }
}

impl wasi::sockets::tcp::HostTcpSocket for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn start_bind(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        network: Resource<wasi::sockets::network::Network>,
        _local_address: wasi::sockets::network::IpSocketAddress,
    ) -> Result<(), errors::NetworkError> {
        // No way to construct TCP socket
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([
            res.rep(),
            network.rep(),
        ]))
        .into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn finish_bind(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn start_connect(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _network: Resource<wasi::sockets::network::Network>,
        _remote_address: wasi::sockets::network::IpSocketAddress,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn finish_connect(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<
        (
            Resource<wasi::io::streams::InputStream>,
            Resource<wasi::io::streams::OutputStream>,
        ),
        errors::NetworkError,
    > {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn start_listen(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn finish_listen(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    #[allow(clippy::type_complexity)]
    fn accept(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<
        (
            Resource<wasi::sockets::tcp::TcpSocket>,
            Resource<wasi::io::streams::InputStream>,
            Resource<wasi::io::streams::OutputStream>,
        ),
        errors::NetworkError,
    > {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn local_address(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn remote_address(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn is_listening(&mut self, res: Resource<wasi::sockets::tcp::TcpSocket>) -> AnyResult<bool> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err)]
    fn address_family(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> AnyResult<wasi::sockets::network::IpAddressFamily> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_listen_backlog_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn keep_alive_enabled(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<bool, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_keep_alive_enabled(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: bool,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn keep_alive_idle_time(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::clocks::monotonic_clock::Duration, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_keep_alive_idle_time(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: wasi::clocks::monotonic_clock::Duration,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn keep_alive_interval(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::clocks::monotonic_clock::Duration, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_keep_alive_interval(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: wasi::clocks::monotonic_clock::Duration,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn keep_alive_count(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u32, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_keep_alive_count(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: u32,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn hop_limit(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u8, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_hop_limit(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: u8,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _value: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn shutdown(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _shutdown_type: wasi::sockets::tcp::ShutdownType,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::sockets::tcp::TcpSocket>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::tcp::Host for WasiContext {}

impl wasi::sockets::udp::HostUdpSocket for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn start_bind(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        network: Resource<wasi::sockets::network::Network>,
        _local_address: wasi::sockets::network::IpSocketAddress,
    ) -> Result<(), errors::NetworkError> {
        // No way to construct UDP socket
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([
            res.rep(),
            network.rep(),
        ]))
        .into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn finish_bind(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn stream(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _remote_address: Option<wasi::sockets::network::IpSocketAddress>,
    ) -> Result<
        (
            Resource<wasi::sockets::udp::IncomingDatagramStream>,
            Resource<wasi::sockets::udp::OutgoingDatagramStream>,
        ),
        errors::NetworkError,
    > {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn local_address(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn remote_address(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn address_family(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> AnyResult<wasi::sockets::network::IpAddressFamily> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn unicast_hop_limit(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<u8, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_unicast_hop_limit(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _value: u8,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _value: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn set_send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _value: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::sockets::udp::UdpSocket>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::udp::HostIncomingDatagramStream for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn receive(
        &mut self,
        res: Resource<wasi::sockets::udp::IncomingDatagramStream>,
        _max_results: u64,
    ) -> Result<Vec<wasi::sockets::udp::IncomingDatagram>, errors::NetworkError> {
        // No way to construct incoming datagram stream
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::udp::IncomingDatagramStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::sockets::udp::IncomingDatagramStream>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::udp::HostOutgoingDatagramStream for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn check_send(
        &mut self,
        res: Resource<wasi::sockets::udp::OutgoingDatagramStream>,
    ) -> Result<u64, errors::NetworkError> {
        // No way to construct outgoing datagram stream
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn send(
        &mut self,
        res: Resource<wasi::sockets::udp::OutgoingDatagramStream>,
        _datagrams: Vec<wasi::sockets::udp::OutgoingDatagram>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    #[instrument(skip(self), err)]
    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::udp::OutgoingDatagramStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::sockets::udp::OutgoingDatagramStream>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::udp::Host for WasiContext {}

impl wasi::sockets::tcp_create_socket::Host for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn create_tcp_socket(
        &mut self,
        _address_family: wasi::sockets::network::IpAddressFamily,
    ) -> Result<Resource<wasi::sockets::tcp::TcpSocket>, errors::NetworkError> {
        Err(AnyError::from(errors::NetworkUnsupportedError).into())
    }
}

impl wasi::sockets::udp_create_socket::Host for WasiContext {
    #[instrument(skip(self), err(level = Level::WARN))]
    fn create_udp_socket(
        &mut self,
        _address_family: wasi::sockets::network::IpAddressFamily,
    ) -> Result<Resource<wasi::sockets::udp::UdpSocket>, errors::NetworkError> {
        Err(AnyError::from(errors::NetworkUnsupportedError).into())
    }
}

impl wasi::cli::stdin::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_stdin(&mut self) -> AnyResult<Resource<wasi::io::streams::InputStream>> {
        let ret: Item = match &mut self.stdin {
            None => NullStdio::default().into(),
            Some(Stdin::Signal((v, _))) => v.clone().into(),
            Some(Stdin::Host(v)) => v.clone().into(),
        };
        self.register(ret)
    }
}

impl wasi::cli::stdout::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_stdout(&mut self) -> AnyResult<Resource<wasi::io::streams::OutputStream>> {
        let ret: Item = match &mut self.stdout {
            None => NullStdio::default().into(),
            Some(v) => v.clone().into(),
        };
        self.register(ret)
    }
}

impl wasi::cli::stderr::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_stderr(&mut self) -> AnyResult<Resource<wasi::io::streams::OutputStream>> {
        let ret: Item = match &mut self.stderr {
            None => NullStdio::default().into(),
            Some(v) => v.clone().into(),
        };
        self.register(ret)
    }
}

impl wasi::cli::terminal_input::HostTerminalInput for WasiContext {
    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::cli::terminal_input::TerminalInput>) -> AnyResult<()> {
        // No way to construct terminal input
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::cli::terminal_input::Host for WasiContext {}

impl wasi::cli::terminal_output::HostTerminalOutput for WasiContext {
    #[instrument(skip(self), err)]
    fn drop(&mut self, res: Resource<wasi::cli::terminal_output::TerminalOutput>) -> AnyResult<()> {
        // No way to construct terminal output
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::cli::terminal_output::Host for WasiContext {}

impl wasi::cli::terminal_stdin::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_terminal_stdin(
        &mut self,
    ) -> AnyResult<Option<Resource<wasi::cli::terminal_input::TerminalInput>>> {
        Ok(None)
    }
}

impl wasi::cli::terminal_stdout::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_terminal_stdout(
        &mut self,
    ) -> AnyResult<Option<Resource<wasi::cli::terminal_output::TerminalOutput>>> {
        Ok(None)
    }
}

impl wasi::cli::terminal_stderr::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_terminal_stderr(
        &mut self,
    ) -> AnyResult<Option<Resource<wasi::cli::terminal_output::TerminalOutput>>> {
        Ok(None)
    }
}

impl wasi::cli::environment::Host for WasiContext {
    #[instrument(skip(self), err)]
    fn get_environment(&mut self) -> AnyResult<Vec<(String, String)>> {
        Ok(self.envs.clone())
    }

    #[instrument(skip(self), err)]
    fn get_arguments(&mut self) -> AnyResult<Vec<String>> {
        Ok(self.args.clone())
    }

    #[instrument(skip(self), err)]
    fn initial_cwd(&mut self) -> AnyResult<Option<String>> {
        Ok(Some(self.cwd.as_path().to_string()))
    }
}

impl wasi::cli::exit::Host for WasiContext {
    #[instrument(skip(self), err(level = Level::INFO))]
    fn exit(&mut self, status: Result<(), ()>) -> AnyResult<()> {
        match status {
            Ok(_) => Err(errors::ProcessExit::default().into()),
            Err(_) => Err(errors::ProcessExit::new(1).into()),
        }
    }

    #[instrument(skip(self), err(level = Level::INFO))]
    fn exit_with_code(&mut self, code: u8) -> AnyResult<()> {
        Err(errors::ProcessExit::new(code.into()).into())
    }
}
