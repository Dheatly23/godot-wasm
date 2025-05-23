use std::borrow::{Borrow, ToOwned};
use std::collections::btree_map::{BTreeMap, Entry};
use std::collections::hash_map::{HashMap, RandomState};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result as AnyResult;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs::Dir as CapDir;
use rand::TryRngCore;
use rand::prelude::*;
use rand::rngs::OsRng;
use rand_xoshiro::Xoshiro512StarStar;
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::clock::{ClockController, UTCClock};
use crate::errors;
use crate::fs_host::{CapWrapper as HostCapWrapper, Descriptor};
use crate::fs_isolated::{AccessMode, CapWrapper, Dir, ILLEGAL_CHARS, IsolatedFSController, Node};
use crate::items::Items;
pub use crate::items::{Item, MaybeBorrowMut};
use crate::preview1::{P1File, P1Item, P1Items};
use crate::stdio::{HostStdin, HostStdout, NullStdio, StdinProvider, StdinSignal};

pub struct WasiContext {
    pub(crate) hasher: RandomState,
    pub(crate) iso_fs: Option<IsolatedFSController>,
    pub(crate) items: Items,
    pub(crate) p1_items: P1Items,
    pub(crate) preopens: Vec<(Utf8PathBuf, FilePreopen)>,
    pub(crate) cwd: Utf8PathBuf,
    pub(crate) envs: Vec<(String, String)>,
    pub(crate) args: Vec<String>,
    pub(crate) clock: ClockController,
    pub(crate) clock_tz: Box<dyn Send + Sync + wasi::clocks::timezone::Host>,
    pub(crate) insecure_rng: Box<dyn Send + Sync + RngCore>,
    pub(crate) secure_rng: Box<dyn Send + Sync + CryptoRng>,
    pub(crate) stdin: Option<Stdin>,
    pub(crate) stdout: Option<Arc<dyn Send + Sync + HostStdout>>,
    pub(crate) stderr: Option<Arc<dyn Send + Sync + HostStdout>>,

    pub(crate) timeout: Option<Instant>,
}

pub struct WasiContextBuilder {
    iso_fs: BuilderIsoFS,
    fs_readonly: bool,
    preopen_dirs: BTreeMap<Utf8PathBuf, (Utf8PathBuf, FilePreopenTy)>,
    cwd: Utf8PathBuf,
    envs: HashMap<String, String>,
    args: Vec<String>,
    clock_tz: Box<dyn Send + Sync + wasi::clocks::timezone::Host>,
    insecure_rng: Option<Box<dyn Send + Sync + RngCore>>,
    secure_rng: Option<Box<dyn Send + Sync + CryptoRng>>,
    stdin: Option<BuilderStdin>,
    stdout: Option<Arc<dyn Send + Sync + HostStdout>>,
    stderr: Option<Arc<dyn Send + Sync + HostStdout>>,
}

enum BuilderIsoFS {
    None,
    New { max_size: usize, max_node: usize },
    Exist(IsolatedFSController),
}

enum BuilderStdin {
    Signal(Box<dyn Fn() + Send + Sync>),
    Host(Arc<dyn Send + Sync + HostStdin>),
}

enum FilePreopenTy {
    IsoFS,
    HostFS,
}

pub(crate) enum FilePreopen {
    IsoFS(CapWrapper),
    HostFS(HostCapWrapper),
}

impl<'a> From<&'a FilePreopen> for Item {
    fn from(v: &'a FilePreopen) -> Item {
        match v {
            FilePreopen::IsoFS(v) => Box::new(v.clone()).into(),
            FilePreopen::HostFS(v) => Box::new(v.clone()).into(),
        }
    }
}

pub(crate) enum Stdin {
    Signal((Arc<StdinSignal>, StdinProvider)),
    Host(Arc<dyn Send + Sync + HostStdin>),
}

impl Default for WasiContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn preopen_dir_iso_fs(
    controller: &IsolatedFSController,
    path: Utf8PathBuf,
) -> AnyResult<Arc<Node>> {
    let mut node = controller.root();
    for c in path.components() {
        node = match c {
            Utf8Component::CurDir => continue,
            Utf8Component::ParentDir | Utf8Component::Prefix(_) => {
                return Err(errors::InvalidPathError(path.into()).into());
            }
            Utf8Component::Normal(s) => {
                let mut n = node.try_dir()?;
                let (m, t) = match n.items.entry(s.into()) {
                    Entry::Vacant(v) => (
                        true,
                        v.insert(Arc::new(Node::from((
                            Dir::new(controller)?,
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
            Utf8Component::RootDir => controller.root(),
        };
    }
    node.try_dir()?;
    Ok(node)
}

fn preopen_dir_host_fs(path: Utf8PathBuf) -> AnyResult<Arc<Descriptor>> {
    Ok(Arc::new(Descriptor::Dir(CapDir::open_ambient_dir(
        path,
        ambient_authority(),
    )?)))
}

fn assert_absolute_path(path: Utf8PathBuf) -> AnyResult<Utf8PathBuf> {
    let mut it = path.components();
    if !matches!(it.next(), Some(Utf8Component::RootDir))
        || it.any(|v| !matches!(v, Utf8Component::Normal(s) if !s.contains(ILLEGAL_CHARS)))
    {
        Err(errors::InvalidPathError(path.into()).into())
    } else {
        Ok(path)
    }
}

impl WasiContextBuilder {
    fn new_iso_fs(&mut self) {
        if matches!(self.iso_fs, BuilderIsoFS::None) {
            self.iso_fs = BuilderIsoFS::New {
                max_size: 0x8000_0000,
                max_node: 0x8000_0000,
            };
        }
    }

    pub fn new() -> Self {
        Self {
            iso_fs: BuilderIsoFS::None,
            fs_readonly: false,
            preopen_dirs: BTreeMap::new(),
            cwd: Utf8PathBuf::new(),
            envs: HashMap::new(),
            args: Vec::new(),
            clock_tz: Box::new(UTCClock),
            insecure_rng: None,
            secure_rng: None,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn max_size(&mut self, size: usize) -> AnyResult<&mut Self> {
        self.new_iso_fs();
        let BuilderIsoFS::New { max_size, .. } = &mut self.iso_fs else {
            return Err(errors::BuilderIsoFSDefinedError.into());
        };
        *max_size = size;
        Ok(self)
    }

    pub fn max_node(&mut self, node: usize) -> AnyResult<&mut Self> {
        self.new_iso_fs();
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

    pub fn preopen_dir_isolated(
        &mut self,
        mut host: Utf8PathBuf,
        guest: Utf8PathBuf,
    ) -> AnyResult<&mut Self> {
        host = assert_absolute_path(host)?;
        match self.preopen_dirs.entry(assert_absolute_path(guest)?) {
            Entry::Occupied(v) => Err(errors::PathAlreadyExistError(v.key().to_string()).into()),
            Entry::Vacant(v) => {
                v.insert((host, FilePreopenTy::IsoFS));
                Ok(self)
            }
        }
    }

    pub fn preopen_dir_host(
        &mut self,
        host: Utf8PathBuf,
        guest: Utf8PathBuf,
    ) -> AnyResult<&mut Self> {
        match self.preopen_dirs.entry(assert_absolute_path(guest)?) {
            Entry::Occupied(v) => Err(errors::PathAlreadyExistError(v.key().to_string()).into()),
            Entry::Vacant(v) => {
                v.insert((host, FilePreopenTy::HostFS));
                Ok(self)
            }
        }
    }

    pub fn clock_timezone(
        &mut self,
        tz: Box<dyn Send + Sync + wasi::clocks::timezone::Host>,
    ) -> &mut Self {
        self.clock_tz = tz;
        self
    }

    pub fn insecure_rng(&mut self, rng: Box<dyn Send + Sync + RngCore>) -> &mut Self {
        self.insecure_rng = Some(rng);
        self
    }

    pub fn secure_rng(&mut self, rng: Box<dyn Send + Sync + CryptoRng>) -> &mut Self {
        self.secure_rng = Some(rng);
        self
    }

    pub fn stdin_signal(&mut self, f: Box<dyn Fn() + Send + Sync>) -> AnyResult<&mut Self> {
        if self.stdin.is_some() {
            return Err(errors::BuilderStdioDefinedError.into());
        }
        self.stdin = Some(BuilderStdin::Signal(f));
        Ok(self)
    }

    pub fn stdin(&mut self, v: Arc<dyn Send + Sync + HostStdin>) -> AnyResult<&mut Self> {
        if self.stdin.is_some() {
            return Err(errors::BuilderStdioDefinedError.into());
        }
        self.stdin = Some(BuilderStdin::Host(v));
        Ok(self)
    }

    pub fn stdout(&mut self, v: Arc<dyn Send + Sync + HostStdout>) -> AnyResult<&mut Self> {
        if self.stdout.is_some() {
            return Err(errors::BuilderStdioDefinedError.into());
        }
        self.stdout = Some(v);
        Ok(self)
    }

    pub fn stderr(&mut self, v: Arc<dyn Send + Sync + HostStdout>) -> AnyResult<&mut Self> {
        if self.stderr.is_some() {
            return Err(errors::BuilderStdioDefinedError.into());
        }
        self.stderr = Some(v);
        Ok(self)
    }

    pub fn env(&mut self, key: String, val: String) -> &mut Self {
        if !key.contains('=') {
            self.envs.insert(key, val);
        }
        self
    }

    pub fn envs(&mut self, it: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.envs
            .extend(it.into_iter().filter(|(k, _)| !k.contains('=')));
        self
    }

    pub fn cwd(
        &mut self,
        cwd: impl Borrow<Utf8Path> + ToOwned<Owned = Utf8PathBuf>,
    ) -> AnyResult<&mut Self> {
        self.cwd = match cwd.borrow().components().next() {
            None => "/".into(),
            Some(Utf8Component::RootDir) => cwd.to_owned(),
            _ => return Err(errors::RelativePathError.into()),
        };
        Ok(self)
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = String>) -> &mut Self {
        self.args.extend(args);
        self
    }

    pub fn build(self) -> AnyResult<WasiContext> {
        let access = if self.fs_readonly {
            AccessMode::R
        } else {
            AccessMode::RW
        };
        let iso_fs = match self.iso_fs {
            BuilderIsoFS::None => None,
            BuilderIsoFS::New { max_size, max_node } => {
                Some(IsolatedFSController::new(max_size, max_node)?)
            }
            BuilderIsoFS::Exist(controller) => Some(controller),
        };

        let preopens = self
            .preopen_dirs
            .into_iter()
            .map(|(dst, (src, ty))| {
                Ok((
                    dst,
                    match ty {
                        FilePreopenTy::IsoFS => FilePreopen::IsoFS(CapWrapper::new(
                            preopen_dir_iso_fs(
                                iso_fs.as_ref().ok_or(errors::BuilderIsoFSNotDefinedError)?,
                                src,
                            )?,
                            access,
                        )),
                        FilePreopenTy::HostFS => FilePreopen::HostFS(HostCapWrapper::new(
                            preopen_dir_host_fs(src)?,
                            access,
                        )),
                    },
                ))
            })
            .collect::<AnyResult<Vec<_>>>()?;

        let mut stdin = self.stdin.map(|v| match v {
            BuilderStdin::Signal(f) => Stdin::Signal(StdinSignal::new(f)),
            BuilderStdin::Host(v) => Stdin::Host(v),
        });

        let p1_items = [
            match &mut stdin {
                None => P1Item::from(NullStdio::default()),
                Some(Stdin::Signal((v, _))) => v.clone().into(),
                Some(Stdin::Host(v)) => v.clone().into(),
            },
            match &self.stdout {
                None => NullStdio::default().into(),
                Some(v) => v.clone().into(),
            },
            match &self.stderr {
                None => NullStdio::default().into(),
                Some(v) => v.clone().into(),
            },
        ]
        .into_iter()
        .chain(preopens.iter().map(|(k, v)| {
            Box::new(P1File::with_preopen(
                match v {
                    FilePreopen::IsoFS(v) => v.clone().into(),
                    FilePreopen::HostFS(v) => v.clone().into(),
                },
                k.as_str().to_string(),
            ))
            .into()
        }))
        .collect::<P1Items>();

        Ok(WasiContext {
            items: Items::new(),
            iso_fs,
            p1_items,
            preopens,
            cwd: self.cwd,
            envs: self.envs.into_iter().collect(),
            args: self.args,
            clock: ClockController::new(),
            clock_tz: self.clock_tz,
            insecure_rng: match self.insecure_rng {
                Some(v) => v,
                None => Box::new(Xoshiro512StarStar::try_from_os_rng()?),
            },
            secure_rng: self
                .secure_rng
                .unwrap_or_else(|| Box::new(OsRng.unwrap_err())),
            stdin,
            stdout: self.stdout,
            stderr: self.stderr,
            hasher: RandomState::new(),
            timeout: None,
        })
    }
}

impl WasiContext {
    #[inline(always)]
    pub fn builder() -> WasiContextBuilder {
        WasiContextBuilder::new()
    }

    #[inline(always)]
    pub fn iso_fs_controller(&self) -> Option<&IsolatedFSController> {
        self.iso_fs.as_ref()
    }

    #[inline(always)]
    pub fn clock_controller(&self) -> &ClockController {
        &self.clock
    }

    #[inline(always)]
    pub fn stdin_provider(&self) -> Option<&StdinProvider> {
        match &self.stdin {
            Some(Stdin::Signal((_, v))) => Some(v),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn p1_items(&mut self) -> &mut P1Items {
        &mut self.p1_items
    }

    #[inline(always)]
    pub fn set_timeout(&mut self, timeout: Instant) {
        self.timeout = Some(timeout);
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

pub(crate) fn try_iso_fs(
    iso_fs: &Option<IsolatedFSController>,
) -> AnyResult<&IsolatedFSController> {
    iso_fs
        .as_ref()
        .ok_or_else(|| errors::BuilderIsoFSNotDefinedError.into())
}
