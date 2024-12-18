use std::collections::btree_map::Entry;
use std::collections::hash_set::HashSet;
use std::convert::{AsMut, AsRef};
use std::io::{Error as IoError, ErrorKind, SeekFrom, Write as _};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Error as AnyError, Result as AnyResult};
use camino::{Utf8Component, Utf8PathBuf};
use rand::prelude::*;
use rand::rngs::OsRng;
use rand_xoshiro::Xoshiro512StarStar;
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::clock::{ClockController, UTCClock};
use crate::fs_isolated::{AccessMode, CapWrapper, Dir, IsolatedFSController, Node, ILLEGAL_CHARS};
use crate::items::Items;
pub use crate::items::{Item, MaybeBorrowMut};
use crate::{errors, items, NullPollable};

pub struct WasiContext {
    pub(crate) items: Items,
    pub(crate) iso_fs: IsolatedFSController,
    pub(crate) preopens: Vec<(Utf8PathBuf, CapWrapper)>,
    pub(crate) clock: ClockController,
    pub(crate) clock_tz: Box<dyn wasi::clocks::timezone::Host>,
    pub(crate) insecure_rng: Box<dyn Send + Sync + RngCore>,
    pub(crate) secure_rng: Box<dyn Send + Sync + RngCore>,
}

pub struct WasiContextBuilder {
    iso_fs: BuilderIsoFS,
    fs_readonly: bool,
    preopen_dirs: HashSet<Utf8PathBuf>,
    clock_tz: Box<dyn wasi::clocks::timezone::Host>,
    insecure_rng: Option<Box<dyn Send + Sync + RngCore>>,
    secure_rng: Option<Box<dyn Send + Sync + RngCore>>,
}

enum BuilderIsoFS {
    New { max_size: usize, max_node: usize },
    Exist(IsolatedFSController),
}

impl Default for WasiContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiContextBuilder {
    pub fn new() -> Self {
        Self {
            iso_fs: BuilderIsoFS::New {
                max_size: 0x8000_0000,
                max_node: 0x8000_0000,
            },
            fs_readonly: false,
            preopen_dirs: HashSet::new(),
            clock_tz: Box::new(UTCClock),
            insecure_rng: None,
            secure_rng: None,
        }
    }

    pub fn max_size(&mut self, size: usize) -> AnyResult<&mut Self> {
        let BuilderIsoFS::New { max_size, .. } = &mut self.iso_fs else {
            return Err(errors::BuilderIsoFSDefinedError.into());
        };
        *max_size = size;
        Ok(self)
    }

    pub fn max_node(&mut self, node: usize) -> AnyResult<&mut Self> {
        let BuilderIsoFS::New { max_node, .. } = &mut self.iso_fs else {
            return Err(errors::BuilderIsoFSDefinedError.into());
        };
        *max_node = node;
        Ok(self)
    }

    pub fn isolated_fs_controller(
        &mut self,
        controller: &IsolatedFSController,
    ) -> AnyResult<&mut Self> {
        if matches!(self.iso_fs, BuilderIsoFS::Exist(_)) {
            return Err(errors::BuilderIsoFSDefinedError.into());
        }
        self.iso_fs = BuilderIsoFS::Exist(controller.dup());
        Ok(self)
    }

    pub fn fs_readonly(&mut self, value: bool) -> &mut Self {
        self.fs_readonly = value;
        self
    }

    pub fn preopen_dir(&mut self, s: Utf8PathBuf) -> AnyResult<&mut Self> {
        for c in s.components() {
            match c {
                Utf8Component::ParentDir | Utf8Component::Prefix(_) => (),
                Utf8Component::Normal(s) if s.contains(ILLEGAL_CHARS) => (),
                _ => continue,
            }
            return Err(errors::InvalidPathError(s.into()).into());
        }
        match self.preopen_dirs.replace(s) {
            Some(s) => Err(errors::PathAlreadyExistError(s.into()).into()),
            None => Ok(self),
        }
    }

    pub fn clock_timezone(&mut self, tz: Box<dyn wasi::clocks::timezone::Host>) -> &mut Self {
        self.clock_tz = tz;
        self
    }

    pub fn insecure_rng(&mut self, rng: impl 'static + Send + Sync + RngCore) -> &mut Self {
        self.insecure_rng = Some(Box::new(rng));
        self
    }

    pub fn secure_rng(
        &mut self,
        rng: impl 'static + Send + Sync + RngCore + CryptoRng,
    ) -> &mut Self {
        self.secure_rng = Some(Box::new(rng));
        self
    }

    pub fn build(self) -> AnyResult<WasiContext> {
        let access = if self.fs_readonly {
            AccessMode::R
        } else {
            AccessMode::RW
        };
        let iso_fs = match self.iso_fs {
            BuilderIsoFS::New { max_size, max_node } => {
                IsolatedFSController::new(max_size, max_node)?
            }
            BuilderIsoFS::Exist(controller) => controller,
        };

        let mut preopens = Vec::with_capacity(self.preopen_dirs.len() + 1);
        preopens.push(("/".into(), CapWrapper::new(iso_fs.root(), access)));

        for s in self.preopen_dirs {
            let mut node = iso_fs.root();
            for c in s.components() {
                node = match c {
                    Utf8Component::CurDir => continue,
                    Utf8Component::ParentDir | Utf8Component::Prefix(_) => {
                        return Err(errors::InvalidPathError(s.into()).into())
                    }
                    Utf8Component::Normal(s) => {
                        let mut n = node.try_dir()?;
                        let (m, t) = match n.items.entry(s.into()) {
                            Entry::Vacant(v) => (
                                true,
                                v.insert(Arc::new(Node::from((
                                    Dir::new(&iso_fs)?,
                                    Arc::downgrade(&node),
                                ))))
                                .clone(),
                            ),
                            Entry::Occupied(v) => (false, v.into_mut().clone()),
                        };
                        if m {
                            n.stamp_mut().modify();
                        } else {
                            n.stamp_mut().access();
                        }
                        t
                    }
                    Utf8Component::RootDir => iso_fs.root(),
                };
            }

            preopens.push((s, CapWrapper::new(node, access)));
        }

        Ok(WasiContext {
            items: Items::new(),
            iso_fs,
            preopens,
            clock: ClockController::new(),
            clock_tz: self.clock_tz,
            insecure_rng: self
                .insecure_rng
                .unwrap_or_else(|| Box::new(Xoshiro512StarStar::from_entropy())),
            secure_rng: self.secure_rng.unwrap_or_else(|| Box::new(OsRng)),
        })
    }
}

impl AsRef<IsolatedFSController> for WasiContext {
    fn as_ref(&self) -> &IsolatedFSController {
        &self.iso_fs
    }
}

impl WasiContext {
    #[inline(always)]
    pub fn builder() -> WasiContextBuilder {
        WasiContextBuilder::new()
    }

    pub fn register<T: 'static>(&mut self, v: impl Into<Item>) -> AnyResult<Resource<T>> {
        let i = self.items.insert(v.into());
        match i.try_into() {
            Ok(i) => Ok(Resource::new_own(i)),
            Err(e) => {
                self.items.remove(i);
                Err(e.into())
            }
        }
    }

    pub fn unregister<T: 'static>(&mut self, res: Resource<T>) -> AnyResult<Item> {
        self.items
            .remove(res.rep().try_into()?)
            .ok_or_else(|| errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    pub fn get_item<T: 'static>(
        &mut self,
        res: Resource<T>,
    ) -> AnyResult<MaybeBorrowMut<'_, Item>> {
        let i = res.rep().try_into()?;
        if res.owned() {
            self.items.remove(i).map(MaybeBorrowMut::from)
        } else {
            self.items.get_mut(i).map(MaybeBorrowMut::from)
        }
        .ok_or_else(|| errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }

    pub fn get_item_ref<T: 'static>(&self, res: Resource<T>) -> AnyResult<&Item> {
        self.items
            .get(res.rep().try_into()?)
            .ok_or_else(|| errors::InvalidResourceIDError::from_iter([res.rep()]).into())
    }
}

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
        Ok(polls
            .into_iter()
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
            .collect())
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
            items::IOStream::IsoFSAccess(mut v) => v.read(len)?,
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
            items::IOStream::IsoFSAccess(mut v) => v.read(len)?,
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
            items::IOStream::IsoFSAccess(mut v) => v.skip(len)? as u64,
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
            items::IOStream::IsoFSAccess(mut v) => v.skip(len)? as u64,
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
            items::IOStream::BoxedRead(_) => NullPollable::new().into(),
            _ => return Err(IoError::from(ErrorKind::InvalidInput).into()),
        };
        self.register(ret)
    }

    fn drop(&mut self, res: Resource<wasi::io::streams::InputStream>) -> AnyResult<()> {
        self.items.get_item(res)?;
        Ok(())
    }
}

static EMPTY_BUF: [u8; 4096] = [0; 4096];

impl wasi::io::streams::HostOutputStream for WasiContext {
    fn check_write(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
    ) -> Result<u64, errors::StreamError> {
        if matches!(
            self.items.get_item(res)?,
            items::IOStream::IsoFSAccess(_)
                | items::IOStream::StdoutBp(_)
                | items::IOStream::StderrBp(_)
                | items::IOStream::StdoutLBuf(_)
                | items::IOStream::StdoutBBuf(_)
        ) {
            Ok(65536)
        } else {
            Err(ErrorKind::InvalidInput.into())
        }
    }

    fn write(
        &mut self,
        res: Resource<wasi::io::streams::OutputStream>,
        data: Vec<u8>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::IOStream::IsoFSAccess(mut v) => v.write(&data)?,
            items::IOStream::StdoutBp(mut v) => v.write_all(&data)?,
            items::IOStream::StderrBp(mut v) => v.write_all(&data)?,
            items::IOStream::StdoutLBuf(mut v) => v.write_all(&data)?,
            items::IOStream::StdoutBBuf(mut v) => v.write_all(&data)?,
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
            items::IOStream::IsoFSAccess(mut v) => v.write(&data)?,
            items::IOStream::StdoutBp(mut v) => {
                v.write_all(&data)?;
                v.flush()?;
            }
            items::IOStream::StderrBp(mut v) => {
                v.write_all(&data)?;
                v.flush()?;
            }
            items::IOStream::StdoutLBuf(mut v) => {
                v.write_all(&data)?;
                v.flush()?;
            }
            items::IOStream::StdoutBBuf(mut v) => {
                v.write_all(&data)?;
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
            items::IOStream::IsoFSAccess(_) => (),
            items::IOStream::StdoutBp(mut v) => v.flush()?,
            items::IOStream::StderrBp(mut v) => v.flush()?,
            items::IOStream::StdoutLBuf(mut v) => v.flush()?,
            items::IOStream::StdoutBBuf(mut v) => v.flush()?,
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
            items::IOStream::StdoutBp(_)
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
                items::IOStream::IsoFSAccess(v) => v.write(data)?,
                items::IOStream::StdoutBp(v) => v.write_all(data)?,
                items::IOStream::StderrBp(v) => v.write_all(data)?,
                items::IOStream::StdoutLBuf(v) => v.write_all(data)?,
                items::IOStream::StdoutBBuf(v) => v.write_all(data)?,
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
                items::IOStream::IsoFSAccess(v) => v.write(data)?,
                items::IOStream::StdoutBp(v) => v.write_all(data)?,
                items::IOStream::StderrBp(v) => v.write_all(data)?,
                items::IOStream::StdoutLBuf(v) => v.write_all(data)?,
                items::IOStream::StdoutBBuf(v) => v.write_all(data)?,
                _ => return Err(ErrorKind::InvalidInput.into()),
            }
            len -= data.len() as u64;
        }

        match v {
            items::IOStream::IsoFSAccess(_) => (),
            items::IOStream::StdoutBp(mut v) => v.flush()?,
            items::IOStream::StderrBp(mut v) => v.flush()?,
            items::IOStream::StdoutLBuf(mut v) => v.flush()?,
            items::IOStream::StdoutBBuf(mut v) => v.flush()?,
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
        if !matches!(
            (&input, &output),
            (
                items::IOStream::IsoFSAccess(_)
                    | items::IOStream::StdinSignal(_)
                    | items::IOStream::BoxedRead(_),
                items::IOStream::IsoFSAccess(_)
                    | items::IOStream::StdoutBp(_)
                    | items::IOStream::StderrBp(_)
                    | items::IOStream::StdoutLBuf(_)
                    | items::IOStream::StdoutBBuf(_)
            )
        ) {
            return Err(ErrorKind::InvalidInput.into());
        }

        let mut n = 0;
        let mut l = usize::try_from(len).map_err(AnyError::from)?;
        while l > 0 {
            let i = l.min(4096);

            let b = match &mut input {
                items::IOStream::IsoFSAccess(v) => v.read(i)?,
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
                items::IOStream::StdoutBp(v) => v.write_all(&b)?,
                items::IOStream::StderrBp(v) => v.write_all(&b)?,
                items::IOStream::StdoutLBuf(v) => v.write_all(&b)?,
                items::IOStream::StdoutBBuf(v) => v.write_all(&b)?,
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
        if !matches!(
            (&input, &output),
            (
                items::IOStream::IsoFSAccess(_)
                    | items::IOStream::StdinSignal(_)
                    | items::IOStream::BoxedRead(_),
                items::IOStream::IsoFSAccess(_)
                    | items::IOStream::StdoutBp(_)
                    | items::IOStream::StderrBp(_)
                    | items::IOStream::StdoutLBuf(_)
                    | items::IOStream::StdoutBBuf(_)
            )
        ) {
            return Err(ErrorKind::InvalidInput.into());
        }

        let mut n = 0;
        let mut l = usize::try_from(len).map_err(AnyError::from)?;
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
                items::IOStream::StdoutBp(v) => v.write_all(&b)?,
                items::IOStream::StderrBp(v) => v.write_all(&b)?,
                items::IOStream::StdoutLBuf(v) => v.write_all(&b)?,
                items::IOStream::StdoutBBuf(v) => v.write_all(&b)?,
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

impl wasi::filesystem::types::HostDescriptor for WasiContext {
    fn read_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<Resource<wasi::io::streams::InputStream>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                Box::new(v.open_file(AccessMode::R, SeekFrom::Start(off))?).into()
            }
        };
        Ok(self.register(ret)?)
    }

    fn write_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<Resource<wasi::io::streams::OutputStream>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                Box::new(v.open_file(AccessMode::W, SeekFrom::Start(off))?).into()
            }
        };
        Ok(self.register(ret)?)
    }

    fn append_via_stream(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<Resource<wasi::io::streams::OutputStream>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                Box::new(v.open_file(AccessMode::W, SeekFrom::End(0))?).into()
            }
        };
        Ok(self.register(ret)?)
    }
    fn advise(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        _: wasi::filesystem::types::Filesize,
        _: wasi::filesystem::types::Filesize,
        _: wasi::filesystem::types::Advice,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) => (),
        }
        Ok(())
    }

    fn sync_data(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) => (),
        }
        Ok(())
    }

    fn get_flags(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::DescriptorFlags, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.file_flags(),
        }
    }

    fn get_type(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::DescriptorType, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.file_type(),
        }
    }

    fn set_size(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        size: wasi::filesystem::types::Filesize,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.resize(size.try_into().map_err(AnyError::from)?)?,
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
            items::Desc::IsoFSNode(v) => v.set_time(|stamp| {
                let now = SystemTime::now();
                set_time(mtime, &now, &mut stamp.mtime);
                set_time(atime, &now, &mut stamp.atime);
                Ok(())
            })?,
        }
        Ok(())
    }

    fn read(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        len: wasi::filesystem::types::Filesize,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<(Vec<u8>, bool), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                let l = usize::try_from(len).unwrap_or(usize::MAX);
                let r = v.read(l, off.try_into().map_err(AnyError::from)?)?;
                let b = r.len() == l;
                Ok((r, b))
            }
        }
    }

    fn write(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
        buf: Vec<u8>,
        off: wasi::filesystem::types::Filesize,
    ) -> Result<wasi::filesystem::types::Filesize, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => {
                v.write(&buf, off.try_into().map_err(AnyError::from)?)?;
                Ok(buf.len() as _)
            }
        }
    }

    fn read_directory(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<Resource<wasi::filesystem::types::DirectoryEntryStream>, errors::StreamError> {
        let ret: Item = match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => Box::new(v.read_directory()?).into(),
        };
        Ok(self.register(ret)?)
    }

    fn sync(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<(), errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(_) => (),
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

                v.open(&self.iso_fs, parent, true, false, false, AccessMode::W)?
                    .create_dir(&self.iso_fs, name)?;
            }
        }
        Ok(())
    }

    fn stat(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::DescriptorStat, errors::StreamError> {
        match self.items.get_item(res)? {
            items::Desc::IsoFSNode(v) => v.stat(),
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
                    &self.iso_fs,
                    &Utf8PathBuf::from(path),
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                    false,
                    false,
                    AccessMode::RW,
                )?
                .stat(),
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
                    &self.iso_fs,
                    &Utf8PathBuf::from(path),
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                    false,
                    false,
                    AccessMode::W,
                )?
                .set_time(|stamp| {
                    let now = SystemTime::now();
                    set_time(mtime, &now, &mut stamp.mtime);
                    set_time(atime, &now, &mut stamp.atime);
                    Ok(())
                })?,
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
            items::Desc::IsoFSNode(_) => Err(ErrorKind::Unsupported.into()),
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
                let symlink =
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW);
                let create = open_flags.contains(wasi::filesystem::types::OpenFlags::CREATE);
                let is_dir = open_flags.contains(
                    wasi::filesystem::types::OpenFlags::CREATE
                        | wasi::filesystem::types::OpenFlags::DIRECTORY,
                );
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
                let v = v.open(
                    &self.iso_fs,
                    &Utf8PathBuf::from(path),
                    symlink,
                    create,
                    is_dir,
                    access,
                )?;

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
                    &self.iso_fs,
                    &Utf8PathBuf::from(path),
                    false,
                    false,
                    false,
                    AccessMode::R,
                )?
                .read_link(),
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

                v.open(&self.iso_fs, parent, true, false, false, AccessMode::W)?
                    .unlink(name, true)?;
            }
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

                let src = src.open(&self.iso_fs, src_path, true, false, false, AccessMode::RW)?;
                let dst = dst.open(&self.iso_fs, dst_path, true, false, false, AccessMode::RW)?;

                dst.move_file(src.node(), src_file, dst_file)?;
            }
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

                v.open(&self.iso_fs, parent, true, false, false, AccessMode::W)?
                    .create_link(&self.iso_fs, name, &Utf8PathBuf::from(target))?;
            }
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

                v.open(&self.iso_fs, parent, true, false, false, AccessMode::W)?
                    .unlink(name, false)?;
            }
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
        };
        self.items.maybe_unregister(res);
        Ok(ret)
    }

    fn metadata_hash(
        &mut self,
        res: Resource<wasi::filesystem::types::Descriptor>,
    ) -> Result<wasi::filesystem::types::MetadataHashValue, errors::StreamError> {
        match self.items.get_item_ref(&res)? {
            items::DescR::IsoFSNode(v) => Ok(v.metadata_hash(&self.iso_fs)),
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
                    &self.iso_fs,
                    &Utf8PathBuf::from(path),
                    path_flags.contains(wasi::filesystem::types::PathFlags::SYMLINK_FOLLOW),
                    false,
                    false,
                    AccessMode::RW,
                )?
                .metadata_hash(&self.iso_fs)),
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
            items::Readdir::IsoFSReaddir(mut v) => Ok(v.next()),
        }
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
                let i = self.items.insert(Box::new(v.clone()).into());
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
            .map_err(AnyError::from)?;
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
