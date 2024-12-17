use std::collections::btree_map::Entry;
use std::collections::hash_set::HashSet;
use std::convert::{AsMut, AsRef};
use std::io::{Error as IoError, ErrorKind, Write as _};
use std::sync::Arc;

use anyhow::{Error as AnyError, Result as AnyResult};
use camino::{Utf8Component, Utf8PathBuf};
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::fs_isolated::{AccessMode, CapWrapper, Dir, IsolatedFSController, Node, ILLEGAL_CHARS};
use crate::items::Items;
pub use crate::items::{Item, MaybeBorrowMut};
use crate::{errors, items, NullPollable};

pub struct WasiContext {
    pub(crate) items: Items,
    pub(crate) iso_fs: IsolatedFSController,
    pub(crate) preopens: Vec<(Utf8PathBuf, CapWrapper)>,
}

pub struct WasiContextBuilder {
    iso_fs: BuilderIsoFS,
    fs_readonly: bool,
    preopen_dirs: HashSet<Utf8PathBuf>,
}

enum BuilderIsoFS {
    New { max_size: usize, max_node: usize },
    Exist(IsolatedFSController),
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
        })
    }

    fn block(&mut self, res: Resource<wasi::io::poll::Pollable>) -> AnyResult<()> {
        match self.items.get_item(res)? {
            items::Poll::NullPoll(_) => (),
            items::Poll::StdinPoll(v) => v.block()?,
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
            match v {
                items::IOStream::IsoFSAccess(ref mut v) => v.write(data)?,
                items::IOStream::StdoutBp(ref mut v) => v.write_all(data)?,
                items::IOStream::StderrBp(ref mut v) => v.write_all(data)?,
                items::IOStream::StdoutLBuf(ref mut v) => v.write_all(data)?,
                items::IOStream::StdoutBBuf(ref mut v) => v.write_all(data)?,
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
            match v {
                items::IOStream::IsoFSAccess(ref mut v) => v.write(data)?,
                items::IOStream::StdoutBp(ref mut v) => v.write_all(data)?,
                items::IOStream::StderrBp(ref mut v) => v.write_all(data)?,
                items::IOStream::StdoutLBuf(ref mut v) => v.write_all(data)?,
                items::IOStream::StdoutBBuf(ref mut v) => v.write_all(data)?,
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

            let b = match input {
                items::IOStream::IsoFSAccess(ref mut v) => v.read(i)?,
                items::IOStream::StdinSignal(ref v) => v.read(i)?,
                items::IOStream::BoxedRead(ref mut v) => {
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

            match output {
                items::IOStream::IsoFSAccess(ref mut v) => v.write(&b)?,
                items::IOStream::StdoutBp(ref mut v) => v.write_all(&b)?,
                items::IOStream::StderrBp(ref mut v) => v.write_all(&b)?,
                items::IOStream::StdoutLBuf(ref mut v) => v.write_all(&b)?,
                items::IOStream::StdoutBBuf(ref mut v) => v.write_all(&b)?,
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

            let b = match input {
                items::IOStream::IsoFSAccess(ref mut v) => v.read(i)?,
                items::IOStream::StdinSignal(ref v) => v.read_block(i)?,
                items::IOStream::BoxedRead(ref mut v) => {
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

            match output {
                items::IOStream::IsoFSAccess(ref mut v) => v.write(&b)?,
                items::IOStream::StdoutBp(ref mut v) => v.write_all(&b)?,
                items::IOStream::StderrBp(ref mut v) => v.write_all(&b)?,
                items::IOStream::StdoutLBuf(ref mut v) => v.write_all(&b)?,
                items::IOStream::StdoutBBuf(ref mut v) => v.write_all(&b)?,
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
