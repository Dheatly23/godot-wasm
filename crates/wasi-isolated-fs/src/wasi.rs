use std::io::{Error as IoError, ErrorKind, Read};
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
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::context::{try_iso_fs, Stderr, Stdin, Stdout, WasiContext};
use crate::fs_host::{CapWrapper as HostCapWrapper, Descriptor};
use crate::fs_isolated::{AccessMode, CreateParams, OpenMode};
use crate::items::Item;
use crate::poll::PollController;
use crate::stdio::NullStdio;
use crate::{errors, items, NullPollable, EMPTY_BUF};

impl wasi::io::poll::HostPollable for WasiContext {
    fn ready(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<bool> {
        Ok(match self.items.get_item(res)? {
            items::Poll::NullPoll(_) => true,
            items::Poll::StdinPoll(v) => v.is_ready(),
            items::Poll::ClockPoll(v) => v.is_ready(),
        })
    }

    fn block(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<()> {
        match self.items.get_item(res)? {
            items::Poll::NullPoll(_) => (),
            items::Poll::StdinPoll(v) => v.block()?,
            items::Poll::ClockPoll(v) => v.block()?,
        }
        Ok(())
    }

    fn drop(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::io::poll::Host for WasiContext {
    fn poll(&mut self, res: Vec<Resource<wasi::io::poll::Pollable>>) -> AnyResult<Vec<u32>> {
        let polls = self.items.get_item(res)?;
        match &*polls {
            [] => return Err(IoError::from(ErrorKind::InvalidInput).into()),
            [v] => {
                match v {
                    items::Poll::NullPoll(_) => (),
                    items::Poll::StdinPoll(v) => v.block()?,
                    items::Poll::ClockPoll(v) => v.block()?,
                }
                return Ok(vec![0]);
            }
            _ => (),
        }

        let mut controller = None;
        for _ in 0..3 {
            let ret: Vec<_> = polls
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if match p {
                        items::Poll::NullPoll(_) => true,
                        items::Poll::StdinPoll(v) => v.is_ready(),
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
                let mut c = PollController::default();
                for i in &polls {
                    match i {
                        items::Poll::NullPoll(_) => (),
                        items::Poll::StdinPoll(v) => c.add_signal(&v.0),
                        items::Poll::ClockPoll(v) => c.set_instant(v.until),
                    }
                }

                c
            });
            c.poll();
        }

        Err(IoError::from(ErrorKind::TimedOut).into())
    }
}

impl wasi::io::error::HostError for WasiContext {
    fn to_debug_string(&mut self, res: Resource<wasi::io::error::Error>) -> AnyResult<String> {
        // No way to construct stream error
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn drop(&mut self, res: Resource<wasi::io::error::Error>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::io::error::Host for WasiContext {}

impl wasi::io::streams::HostInputStream for WasiContext {
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
            items::IOStream::BoxedRead(mut v) => {
                let mut ret = vec![0; len.min(1024)];
                let i = v.read(&mut ret)?;
                ret.truncate(i);
                ret
            }
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

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
            items::IOStream::StdinSignal(v) => v.read(len)?,
            items::IOStream::BoxedRead(mut v) => {
                let mut ret = vec![0; len.min(1024)];
                let i = v.read(&mut ret)?;
                ret.truncate(i);
                ret
            }
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

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
            items::IOStream::BoxedRead(mut v) => v.read(&mut vec![0; len.min(1024)])? as u64,
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

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
            items::IOStream::StdinSignal(v) => v.skip_block(len)? as u64,
            items::IOStream::BoxedRead(mut v) => v.read(&mut vec![0; len.min(1024)])? as u64,
            _ => return Err(ErrorKind::InvalidInput.into()),
        })
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::io::streams::InputStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        let ret: Item = match self.items.get_item(res)? {
            items::IOStream::IsoFSAccess(v) => v.poll()?.into(),
            items::IOStream::StdinSignal(v) => v.poll()?.into(),
            items::IOStream::HostFSStream(_)
            | items::IOStream::BoxedRead(_)
            | items::IOStream::NullStdio(_) => NullPollable::new().into(),
            _ => return Err(IoError::from(ErrorKind::InvalidInput).into()),
        };
        self.register(ret)
    }

    fn drop(&mut self, res: Resource<wasi::io::streams::InputStream>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::io::streams::HostOutputStream for WasiContext {
    fn check_write(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<u64, errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => Ok(0),
            items::IOStream::IsoFSAccess(_)
            | items::IOStream::HostFSStream(_)
            | items::IOStream::StdoutBp(_)
            | items::IOStream::StderrBp(_)
            | items::IOStream::StdoutLBuf(_)
            | items::IOStream::StdoutBBuf(_) => Ok(65536),
            _ => Err(ErrorKind::InvalidInput.into()),
        }
    }

    fn write(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        data: Vec<u8>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => (),
            items::IOStream::IsoFSAccess(mut v) => v.write(&data)?,
            items::IOStream::HostFSStream(mut v) => v.write(&data)?,
            items::IOStream::StdoutBp(v) => v.write(&data)?,
            items::IOStream::StderrBp(v) => v.write(&data)?,
            items::IOStream::StdoutLBuf(v) => v.write(&data)?,
            items::IOStream::StdoutBBuf(v) => v.write(&data)?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    fn blocking_write_and_flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        data: Vec<u8>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_) => (),
            items::IOStream::IsoFSAccess(mut v) => v.write(&data)?,
            items::IOStream::HostFSStream(mut v) => v.write(&data)?,
            items::IOStream::StdoutBp(v) => {
                v.write(&data)?;
                v.flush()?;
            }
            items::IOStream::StderrBp(v) => {
                v.write(&data)?;
                v.flush()?;
            }
            items::IOStream::StdoutLBuf(v) => {
                v.write(&data)?;
                v.flush()?;
            }
            items::IOStream::StdoutBBuf(v) => {
                v.write(&data)?;
                v.flush()?;
            }
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    fn flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<(), errors::StreamError> {
        self.blocking_flush(res)
    }

    fn blocking_flush(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::NullStdio(_)
            | items::IOStream::IsoFSAccess(_)
            | items::IOStream::HostFSStream(_) => (),
            items::IOStream::StdoutBp(v) => v.flush()?,
            items::IOStream::StderrBp(v) => v.flush()?,
            items::IOStream::StdoutLBuf(v) => v.flush()?,
            items::IOStream::StdoutBBuf(v) => v.flush()?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        let ret: Item = match self.items.get_item(res)? {
            items::IOStream::IsoFSAccess(v) => v.poll()?.into(),
            items::IOStream::NullStdio(_)
            | items::IOStream::HostFSStream(_)
            | items::IOStream::StdoutBp(_)
            | items::IOStream::StderrBp(_)
            | items::IOStream::StdoutLBuf(_)
            | items::IOStream::StdoutBBuf(_) => NullPollable::new().into(),
            _ => return Err(IoError::from(ErrorKind::InvalidInput).into()),
        };
        self.register(ret)
    }

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
                items::IOStream::StdoutBp(v) => v.write(data)?,
                items::IOStream::StderrBp(v) => v.write(data)?,
                items::IOStream::StdoutLBuf(v) => v.write(data)?,
                items::IOStream::StdoutBBuf(v) => v.write(data)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
            len -= data.len() as u64;
        }
        Ok(())
    }

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
                items::IOStream::StdoutBp(v) => v.write(data)?,
                items::IOStream::StderrBp(v) => v.write(data)?,
                items::IOStream::StdoutLBuf(v) => v.write(data)?,
                items::IOStream::StdoutBBuf(v) => v.write(data)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
            len -= data.len() as u64;
        }

        match v {
            items::IOStream::NullStdio(_)
            | items::IOStream::IsoFSAccess(_)
            | items::IOStream::HostFSStream(_) => (),
            items::IOStream::StdoutBp(v) => v.flush()?,
            items::IOStream::StderrBp(v) => v.flush()?,
            items::IOStream::StdoutLBuf(v) => v.flush()?,
            items::IOStream::StdoutBBuf(v) => v.flush()?,
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        Ok(())
    }

    fn splice(
        &mut self,
        output: Resource<wasi::io::streams::OutputStream>,
        input: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<u64, errors::StreamError> {
        let (mut input, mut output) = self.items.get_item((input, output))?;
        match (&input, &output) {
            (items::IOStream::NullStdio(_), _) | (_, items::IOStream::NullStdio(_)) => {
                return Ok(0)
            }
            (
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::HostFSStream(_)
                | items::IOStream::StdinSignal(_)
                | items::IOStream::BoxedRead(_),
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::HostFSStream(_)
                | items::IOStream::StdoutBp(_)
                | items::IOStream::StderrBp(_)
                | items::IOStream::StdoutLBuf(_)
                | items::IOStream::StdoutBBuf(_),
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
                items::IOStream::BoxedRead(v) => {
                    let mut r = vec![0; i.min(1024)];
                    let i = v.read(&mut r)?;
                    r.truncate(i);
                    r
                }
                _ => return Err(ErrorKind::InvalidInput.into()),
            };
            if b.is_empty() {
                break;
            }
            l -= b.len();
            n += b.len();

            match &mut output {
                items::IOStream::IsoFSAccess(v) => v.write(&b)?,
                items::IOStream::StdoutBp(v) => v.write(&b)?,
                items::IOStream::StderrBp(v) => v.write(&b)?,
                items::IOStream::StdoutLBuf(v) => v.write(&b)?,
                items::IOStream::StdoutBBuf(v) => v.write(&b)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
        }

        Ok(n as u64)
    }

    fn blocking_splice(
        &mut self,
        output: Resource<wasi::io::streams::OutputStream>,
        input: Resource<wasi::io::streams::InputStream>,
        len: u64,
    ) -> Result<u64, errors::StreamError> {
        let (mut input, mut output) = self.items.get_item((input, output))?;
        match (&input, &output) {
            (items::IOStream::NullStdio(_), _) | (_, items::IOStream::NullStdio(_)) => {
                return Ok(0)
            }
            (
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::StdinSignal(_)
                | items::IOStream::BoxedRead(_),
                items::IOStream::IsoFSAccess(_)
                | items::IOStream::StdoutBp(_)
                | items::IOStream::StderrBp(_)
                | items::IOStream::StdoutLBuf(_)
                | items::IOStream::StdoutBBuf(_),
            ) => (),
            _ => return Err(ErrorKind::InvalidInput.into()),
        }

        let mut n = 0;
        let mut l = usize::try_from(len).unwrap_or(usize::MAX);
        while l > 0 {
            let i = l.min(4096);

            let b = match &mut input {
                items::IOStream::IsoFSAccess(v) => v.read(i)?,
                items::IOStream::StdinSignal(v) => v.read_block(i)?,
                items::IOStream::BoxedRead(v) => {
                    let mut r = vec![0; i.min(1024)];
                    let i = v.read(&mut r)?;
                    r.truncate(i);
                    r
                }
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
                items::IOStream::StdoutBp(v) => v.write(&b)?,
                items::IOStream::StderrBp(v) => v.write(&b)?,
                items::IOStream::StdoutLBuf(v) => v.write(&b)?,
                items::IOStream::StdoutBBuf(v) => v.write(&b)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
        }

        Ok(n as u64)
    }

    fn drop(&mut self, res: Resource<wasi::io::streams::OutputStream>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::io::streams::Host for WasiContext {
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
    fn read_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<Resource<wasi::io::streams::InputStream>, errors::StreamError> {
        self.open_file(res, OpenMode::Read(off.try_into()?))
    }

    fn write_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<Resource<wasi::io::streams::OutputStream>, errors::StreamError> {
        self.open_file(res, OpenMode::Write(off.try_into()?))
    }

    fn append_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<Resource<wasi::io::streams::OutputStream>, errors::StreamError> {
        self.open_file(res, OpenMode::Append)
    }

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

    fn link_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        _: wasi::filesystem::types::PathFlags,
        _: String,
        _: Resource<wasi::filesystem::types::Descriptor>,
        _: String,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) | items::Desc::HostFSDesc(_) => {
                Err(ErrorKind::Unsupported.into())
            }
        }
    }

    fn open_at(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        path_flags: wasi::filesystem::types::PathFlags,
        path: String,
        open_flags: wasi::filesystem::types::OpenFlags,
        flags: wasi::filesystem::types::DescriptorFlags,
    ) -> Result<Resource<wasi::filesystem::types::Descriptor>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                if open_flags.contains(
                    wasi::filesystem::types::OpenFlags::DIRECTORY
                        | wasi::filesystem::types::OpenFlags::TRUNCATE,
                ) {
                    return Err(ErrorKind::InvalidInput.into());
                }

                let symlink =
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW);
                let create = if open_flags.contains(wasi::filesystem::types::OpenFlags::CREATE) {
                    Some(CreateParams {
                        dir: open_flags.contains(wasi::filesystem::types::OpenFlags::DIRECTORY),
                        exclusive: open_flags
                            .contains(wasi::filesystem::types::OpenFlags::EXCLUSIVE),
                    })
                } else {
                    None
                };
                let access = match (
                    flags.contains(wasi::filesystem::types::DescriptorFlags::READ),
                    flags.intersects(
                        wasi::filesystem::types::DescriptorFlags::WRITE
                            | wasi::filesystem::types::DescriptorFlags::MUTATE_DIRECTORY,
                    ),
                ) {
                    (false, false) => AccessMode::NA,
                    (true, false) => AccessMode::R,
                    (false, true) => AccessMode::W,
                    (true, true) => AccessMode::RW,
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

                if flags.contains(wasi::filesystem::types::DescriptorFlags::MUTATE_DIRECTORY)
                    && !v.node().is_dir()
                {
                    return Err(ErrorKind::PermissionDenied.into());
                }
                if open_flags.contains(wasi::filesystem::types::OpenFlags::TRUNCATE) {
                    v.resize(0)?;
                }

                Box::new(v).into()
            }
            items::Desc::HostFSDesc(v) => {
                if open_flags.contains(
                    wasi::filesystem::types::OpenFlags::DIRECTORY
                        | wasi::filesystem::types::OpenFlags::TRUNCATE,
                ) {
                    return Err(ErrorKind::InvalidInput.into());
                }

                let access = match (
                    flags.contains(wasi::filesystem::types::DescriptorFlags::READ),
                    flags.intersects(
                        wasi::filesystem::types::DescriptorFlags::WRITE
                            | wasi::filesystem::types::DescriptorFlags::MUTATE_DIRECTORY,
                    ),
                ) {
                    (false, false) => AccessMode::NA,
                    (true, false) => AccessMode::R,
                    (false, true) => AccessMode::W,
                    (true, true) => AccessMode::RW,
                } & v.access();

                let mut opts = OpenOptions::new();
                if open_flags.contains(wasi::filesystem::types::OpenFlags::CREATE) {
                    access.write_or_err()?;
                    if open_flags.contains(wasi::filesystem::types::OpenFlags::EXCLUSIVE) {
                        opts.create_new(true);
                    } else {
                        opts.create(true);
                    }
                }
                match access {
                    AccessMode::NA | AccessMode::R => opts.read(true),
                    AccessMode::W => opts.write(true),
                    AccessMode::RW => opts.read(true).write(true),
                };
                if open_flags.contains(wasi::filesystem::types::OpenFlags::TRUNCATE) {
                    opts.truncate(true);
                }
                if path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW) {
                    opts.follow(FollowSymlinks::Yes);
                } else {
                    opts.follow(FollowSymlinks::No);
                }
                let is_dir = open_flags.contains(wasi::filesystem::types::OpenFlags::DIRECTORY);
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

    fn metadata_hash(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::MetadataHashValue, errors::StreamError> {
        match self.items.get_item_ref(&res)? {
            items::DescR::IsoFSNode(v) => Ok(v.metadata_hash(&self.hasher)),
            items::DescR::HostFSDesc(v) => v.metadata_hash(&self.hasher),
        }
    }

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

    fn drop(&mut self, res: Resource<wasi::filesystem::types::Descriptor>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::filesystem::types::HostDirectoryEntryStream for WasiContext {
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

    fn drop(
        &mut self,
        res: Resource<wasi::filesystem::types::DirectoryEntryStream>,
    ) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

impl wasi::filesystem::types::Host for WasiContext {
    fn filesystem_error_code(
        &mut self,
        res: Resource<wasi::filesystem::types::Error>,
    ) -> AnyResult<Option<wasi::filesystem::types::ErrorCode>> {
        // No way to construct stream error
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn convert_error_code(
        &mut self,
        e: errors::StreamError,
    ) -> AnyResult<wasi::filesystem::types::ErrorCode> {
        e.into()
    }
}

impl wasi::filesystem::preopens::Host for WasiContext {
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
    fn now(&mut self) -> AnyResult<wasi::clocks::monotonic_clock::Instant> {
        Ok(self.clock.now())
    }

    fn resolution(&mut self) -> AnyResult<wasi::clocks::monotonic_clock::Duration> {
        Ok(1000)
    }

    fn subscribe_instant(
        &mut self,
        when: wasi::clocks::monotonic_clock::Instant,
    ) -> AnyResult<Resource<wasi::clocks::monotonic_clock::Pollable>> {
        let ret = Item::from(Box::new(self.clock.poll_until(when)?));
        self.register(ret)
    }

    fn subscribe_duration(
        &mut self,
        when: wasi::clocks::monotonic_clock::Duration,
    ) -> AnyResult<Resource<wasi::clocks::monotonic_clock::Pollable>> {
        let ret = Item::from(Box::new(self.clock.poll_for(when)?));
        self.register(ret)
    }
}

impl wasi::clocks::wall_clock::Host for WasiContext {
    fn now(&mut self) -> AnyResult<wasi::clocks::wall_clock::Datetime> {
        let t = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        Ok(wasi::clocks::wall_clock::Datetime {
            seconds: t.as_secs(),
            nanoseconds: t.subsec_nanos(),
        })
    }

    fn resolution(&mut self) -> AnyResult<wasi::clocks::wall_clock::Datetime> {
        Ok(wasi::clocks::wall_clock::Datetime {
            seconds: 0,
            nanoseconds: 1000,
        })
    }
}

impl wasi::clocks::timezone::Host for WasiContext {
    fn display(
        &mut self,
        time: wasi::clocks::timezone::Datetime,
    ) -> AnyResult<wasi::clocks::timezone::TimezoneDisplay> {
        self.clock_tz.display(time)
    }

    fn utc_offset(&mut self, time: wasi::clocks::timezone::Datetime) -> AnyResult<i32> {
        self.clock_tz.utc_offset(time)
    }
}

impl wasi::random::insecure::Host for WasiContext {
    fn get_insecure_random_bytes(&mut self, len: u64) -> AnyResult<Vec<u8>> {
        let mut ret = vec![0u8; len.try_into()?];
        self.insecure_rng.fill(&mut ret[..]);
        Ok(ret)
    }

    fn get_insecure_random_u64(&mut self) -> AnyResult<u64> {
        Ok(self.insecure_rng.gen())
    }
}

impl wasi::random::insecure_seed::Host for WasiContext {
    fn insecure_seed(&mut self) -> AnyResult<(u64, u64)> {
        Ok(self.insecure_rng.gen())
    }
}

impl wasi::random::random::Host for WasiContext {
    fn get_random_bytes(&mut self, len: u64) -> AnyResult<Vec<u8>> {
        let mut ret = vec![0u8; len.try_into()?];
        self.secure_rng.fill(&mut ret[..]);
        Ok(ret)
    }

    fn get_random_u64(&mut self) -> AnyResult<u64> {
        Ok(self.secure_rng.gen())
    }
}

impl wasi::sockets::network::HostNetwork for WasiContext {
    fn drop(&mut self, res: Resource<wasi::sockets::network::Network>) -> AnyResult<()> {
        // No way to construct network connection
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::network::Host for WasiContext {
    fn network_error_code(
        &mut self,
        res: Resource<wasi::sockets::network::Error>,
    ) -> AnyResult<Option<wasi::sockets::network::ErrorCode>> {
        // No way to construct network error
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn convert_error_code(
        &mut self,
        e: errors::NetworkError,
    ) -> AnyResult<wasi::sockets::network::ErrorCode> {
        e.into()
    }
}

impl wasi::sockets::instance_network::Host for WasiContext {
    fn instance_network(&mut self) -> AnyResult<Resource<wasi::sockets::network::Network>> {
        Err(errors::NetworkUnsupportedError.into())
    }
}

impl wasi::sockets::ip_name_lookup::HostResolveAddressStream for WasiContext {
    fn resolve_next_address(
        &mut self,
        res: Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>,
    ) -> Result<Option<wasi::sockets::network::IpAddress>, errors::NetworkError> {
        // No way to construct resolve address
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn drop(
        &mut self,
        res: Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>,
    ) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::ip_name_lookup::Host for WasiContext {
    fn resolve_addresses(
        &mut self,
        res: Resource<wasi::sockets::network::Network>,
        _: String,
    ) -> Result<Resource<wasi::sockets::ip_name_lookup::ResolveAddressStream>, errors::NetworkError>
    {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }
}

impl wasi::sockets::tcp::HostTcpSocket for WasiContext {
    fn start_bind(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        network: Resource<wasi::sockets::network::Network>,
        _: wasi::sockets::network::IpSocketAddress,
    ) -> Result<(), errors::NetworkError> {
        // No way to construct TCP socket
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([
            res.rep(),
            network.rep(),
        ]))
        .into())
    }

    fn finish_bind(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn start_connect(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: Resource<wasi::sockets::network::Network>,
        _: wasi::sockets::network::IpSocketAddress,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

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

    fn start_listen(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn finish_listen(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

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

    fn local_address(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn remote_address(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn is_listening(&mut self, res: Resource<wasi::sockets::tcp::TcpSocket>) -> AnyResult<bool> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn address_family(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> AnyResult<wasi::sockets::network::IpAddressFamily> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn set_listen_backlog_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn keep_alive_enabled(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<bool, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_keep_alive_enabled(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: bool,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn keep_alive_idle_time(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::clocks::monotonic_clock::Duration, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_keep_alive_idle_time(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: wasi::clocks::monotonic_clock::Duration,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn keep_alive_interval(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<wasi::clocks::monotonic_clock::Duration, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_keep_alive_interval(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: wasi::clocks::monotonic_clock::Duration,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn keep_alive_count(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u32, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_keep_alive_count(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: u32,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn hop_limit(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u8, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_hop_limit(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: u8,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn shutdown(
        &mut self,
        res: Resource<wasi::sockets::tcp::TcpSocket>,
        _: wasi::sockets::tcp::ShutdownType,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn drop(&mut self, res: Resource<wasi::sockets::tcp::TcpSocket>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::tcp::Host for WasiContext {}

impl wasi::sockets::udp::HostUdpSocket for WasiContext {
    fn start_bind(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        network: Resource<wasi::sockets::network::Network>,
        _: wasi::sockets::network::IpSocketAddress,
    ) -> Result<(), errors::NetworkError> {
        // No way to construct UDP socket
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([
            res.rep(),
            network.rep(),
        ]))
        .into())
    }

    fn finish_bind(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn stream(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _: Option<wasi::sockets::network::IpSocketAddress>,
    ) -> Result<
        (
            Resource<wasi::sockets::udp::IncomingDatagramStream>,
            Resource<wasi::sockets::udp::OutgoingDatagramStream>,
        ),
        errors::NetworkError,
    > {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn local_address(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn remote_address(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<wasi::sockets::network::IpSocketAddress, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn address_family(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> AnyResult<wasi::sockets::network::IpAddressFamily> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn unicast_hop_limit(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<u8, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_unicast_hop_limit(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _: u8,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_receive_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn set_send_buffer_size(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
        _: u64,
    ) -> Result<(), errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::udp::UdpSocket>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn drop(&mut self, res: Resource<wasi::sockets::udp::UdpSocket>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::udp::HostIncomingDatagramStream for WasiContext {
    fn receive(
        &mut self,
        res: Resource<wasi::sockets::udp::IncomingDatagramStream>,
        _: u64,
    ) -> Result<Vec<wasi::sockets::udp::IncomingDatagram>, errors::NetworkError> {
        // No way to construct incoming datagram stream
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::udp::IncomingDatagramStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn drop(&mut self, res: Resource<wasi::sockets::udp::IncomingDatagramStream>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::udp::HostOutgoingDatagramStream for WasiContext {
    fn check_send(
        &mut self,
        res: Resource<wasi::sockets::udp::OutgoingDatagramStream>,
    ) -> Result<u64, errors::NetworkError> {
        // No way to construct outgoing datagram stream
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn send(
        &mut self,
        res: Resource<wasi::sockets::udp::OutgoingDatagramStream>,
        _: Vec<wasi::sockets::udp::OutgoingDatagram>,
    ) -> Result<u64, errors::NetworkError> {
        Err(AnyError::from(errors::InvalidResourceIDError::from_iter([res.rep()])).into())
    }

    fn subscribe(
        &mut self,
        res: Resource<wasi::sockets::udp::OutgoingDatagramStream>,
    ) -> AnyResult<Resource<wasi::io::poll::Pollable>> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    fn drop(&mut self, res: Resource<wasi::sockets::udp::OutgoingDatagramStream>) -> AnyResult<()> {
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::sockets::udp::Host for WasiContext {}

impl wasi::sockets::tcp_create_socket::Host for WasiContext {
    fn create_tcp_socket(
        &mut self,
        _: wasi::sockets::network::IpAddressFamily,
    ) -> Result<Resource<wasi::sockets::tcp::TcpSocket>, errors::NetworkError> {
        Err(AnyError::from(errors::NetworkUnsupportedError).into())
    }
}

impl wasi::sockets::udp_create_socket::Host for WasiContext {
    fn create_udp_socket(
        &mut self,
        _: wasi::sockets::network::IpAddressFamily,
    ) -> Result<Resource<wasi::sockets::udp::UdpSocket>, errors::NetworkError> {
        Err(AnyError::from(errors::NetworkUnsupportedError).into())
    }
}

impl wasi::cli::stdin::Host for WasiContext {
    fn get_stdin(&mut self) -> AnyResult<Resource<wasi::io::streams::InputStream>> {
        let ret: Item = match &mut self.stdin {
            None => NullStdio::default().into(),
            Some(Stdin::Signal((v, _))) => v.clone().into(),
            Some(Stdin::Read(v)) => v()?.into(),
        };
        self.register(ret)
    }
}

impl wasi::cli::stdout::Host for WasiContext {
    fn get_stdout(&mut self) -> AnyResult<Resource<wasi::io::streams::OutputStream>> {
        let ret: Item = match &mut self.stdout {
            None => NullStdio::default().into(),
            Some(Stdout::Bypass(v)) => v.clone().into(),
            Some(Stdout::CbLine(v)) => v.clone().into(),
            Some(Stdout::CbBlock(v)) => v.clone().into(),
        };
        self.register(ret)
    }
}

impl wasi::cli::stderr::Host for WasiContext {
    fn get_stderr(&mut self) -> AnyResult<Resource<wasi::io::streams::OutputStream>> {
        let ret: Item = match &mut self.stderr {
            None => NullStdio::default().into(),
            Some(Stderr::Bypass(v)) => v.clone().into(),
            Some(Stderr::CbLine(v)) => v.clone().into(),
            Some(Stderr::CbBlock(v)) => v.clone().into(),
        };
        self.register(ret)
    }
}

impl wasi::cli::terminal_input::HostTerminalInput for WasiContext {
    fn drop(&mut self, res: Resource<wasi::cli::terminal_input::TerminalInput>) -> AnyResult<()> {
        // No way to construct terminal input
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::cli::terminal_input::Host for WasiContext {}

impl wasi::cli::terminal_output::HostTerminalOutput for WasiContext {
    fn drop(&mut self, res: Resource<wasi::cli::terminal_output::TerminalOutput>) -> AnyResult<()> {
        // No way to construct terminal output
        Err(errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

impl wasi::cli::terminal_output::Host for WasiContext {}

impl wasi::cli::terminal_stdin::Host for WasiContext {
    fn get_terminal_stdin(
        &mut self,
    ) -> AnyResult<Option<Resource<wasi::cli::terminal_input::TerminalInput>>> {
        Ok(None)
    }
}

impl wasi::cli::terminal_stdout::Host for WasiContext {
    fn get_terminal_stdout(
        &mut self,
    ) -> AnyResult<Option<Resource<wasi::cli::terminal_output::TerminalOutput>>> {
        Ok(None)
    }
}

impl wasi::cli::terminal_stderr::Host for WasiContext {
    fn get_terminal_stderr(
        &mut self,
    ) -> AnyResult<Option<Resource<wasi::cli::terminal_output::TerminalOutput>>> {
        Ok(None)
    }
}

impl wasi::cli::environment::Host for WasiContext {
    fn get_environment(&mut self) -> AnyResult<Vec<(String, String)>> {
        Ok(self.envs.clone())
    }

    fn get_arguments(&mut self) -> AnyResult<Vec<String>> {
        Ok(self.args.clone())
    }

    fn initial_cwd(&mut self) -> AnyResult<Option<String>> {
        Ok(Some(self.cwd.as_path().to_string()))
    }
}

impl wasi::cli::exit::Host for WasiContext {
    fn exit(&mut self, status: Result<(), ()>) -> AnyResult<()> {
        match status {
            Ok(_) => Err(errors::ProcessExit::default().into()),
            Err(_) => Err(errors::ProcessExit::new(1).into()),
        }
    }

    fn exit_with_code(&mut self, code: u8) -> AnyResult<()> {
        Err(errors::ProcessExit::new(code.into()).into())
    }
}
