use std::collections::btree_map::Entry;
use std::collections::hash_set::HashSet;
use std::convert::{AsMut, AsRef};
use std::sync::Arc;

use anyhow::Result as AnyResult;
use camino::{Utf8Component, Utf8PathBuf};
use wasmtime::component::Resource;

use crate::errors;
use crate::fs_isolated::{AccessMode, CapWrapper, Dir, IsolatedFSController, Node, ILLEGAL_CHARS};
use crate::items::Items;
pub use crate::items::{Item, MaybeBorrowMut};

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
