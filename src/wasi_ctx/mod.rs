mod memfs;
mod stdio;
mod timestamp;

use std::collections::btree_map::Entry;
use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::mem;
use std::path::{Component, Path, PathBuf};
use std::slice;
use std::sync::{Arc, Weak};

use anyhow::Error;
use gdnative::log::{error, godot_site, Site};
use gdnative::prelude::*;
use wasi_common::dir::OpenResult;
use wasi_common::file::{FdFlags, OFlags};
use wasmtime_wasi::dir::{Dir as CapDir, OpenResult as OpenResult2};
use wasmtime_wasi::{ambient_authority, Dir as PhysicalDir, WasiCtx, WasiCtxBuilder};

use crate::wasi_ctx::memfs::{Capability, Dir, File, Node};
use crate::wasi_ctx::stdio::{ContextStderr, ContextStdout};
use crate::wasm_config::Config;
use crate::{bail_with_site, site_context};

#[derive(NativeClass, Debug)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::RwLockData<WasiContext>)]
pub struct WasiContext {
    bypass_stdio: bool,
    fs_readonly: bool,

    memfs_root: Arc<Dir>,
    physical_mount: HashMap<PathBuf, PathBuf>,
    envs: HashMap<String, String>,
}

impl WasiContext {
    fn new(_owner: &Reference) -> Self {
        Self {
            bypass_stdio: false,
            fs_readonly: false,

            memfs_root: Arc::new(Dir::new(<Weak<Dir>>::new())),
            physical_mount: HashMap::new(),
            envs: HashMap::new(),
        }
    }

    pub fn init_ctx_no_context(mut ctx: WasiCtx, config: &Config) -> Result<WasiCtx, Error> {
        for (k, v) in &config.wasi_envs {
            ctx.push_env(k, v)?;
        }

        for a in &config.wasi_args {
            ctx.push_arg(a)?;
        }

        Ok(ctx)
    }

    pub fn build_ctx(
        this: Instance<Self>,
        ctx: WasiCtxBuilder,
        config: &Config,
    ) -> Result<WasiCtx, Error> {
        unsafe {
            this.assume_safe().map(move |o, b| -> Result<_, Error> {
                let mut ctx = ctx;
                if o.bypass_stdio {
                    ctx = ctx.inherit_stdout().inherit_stderr();
                } else {
                    ctx = ctx
                        .stdout(Box::new(ContextStdout::new(b.claim())))
                        .stderr(Box::new(ContextStderr::new(b.claim())));
                }

                let mut ctx = Self::init_ctx_no_context(ctx.build(), config)?;

                for (k, v) in o
                    .envs
                    .iter()
                    .filter(|(k, _)| !config.wasi_envs.contains_key(&**k))
                {
                    ctx.push_env(k, v)?;
                }

                let fs_writable = !(o.fs_readonly || config.wasi_fs_readonly);

                for (guest, host) in o.physical_mount.iter() {
                    let dir = CapDir::from_cap_std(site_context!(PhysicalDir::open_ambient_dir(
                        host,
                        ambient_authority(),
                    ))?);
                    let OpenResult2::Dir(dir) = site_context!(dir.open_file_(
                        false,
                        ".",
                        OFlags::DIRECTORY,
                        true,
                        fs_writable,
                        FdFlags::empty(),
                    ))? else { bail_with_site!("Path should be a directory!") };
                    site_context!(ctx.push_preopened_dir(Box::new(dir), guest))?;
                }

                let OpenResult::Dir(root) = site_context!(o.memfs_root.clone().open(
                    Some(o.memfs_root.clone()),
                    Capability {
                        read: true,
                        write: fs_writable,
                    },
                    true,
                    OFlags::DIRECTORY,
                    FdFlags::empty(),
                ))? else { bail_with_site!("Root should be a directory!") };
                site_context!(ctx.push_preopened_dir(root, "."))?;

                Ok(ctx)
            })?
        }
    }

    fn wrap_result<F, T>(f: F) -> Option<T>
    where
        F: FnOnce() -> Result<T, Error>,
    {
        match f() {
            Ok(v) => Some(v),
            Err(e) => {
                let s = format!("{:?}", e);
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    s,
                );
                None
            }
        }
    }
}

#[methods]
impl WasiContext {
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .property("fs_readonly")
            .with_getter(|this, _| this.fs_readonly)
            .with_setter(|this, _, v| this.fs_readonly = v)
            .done();

        builder
            .property("bypass_stdio")
            .with_getter(|this, _| this.bypass_stdio)
            .with_setter(|this, _, v| this.bypass_stdio = v)
            .done();

        builder
            .signal("stdout_emit")
            .with_param("message", VariantType::ByteArray)
            .done();

        builder
            .signal("stderr_emit")
            .with_param("message", VariantType::ByteArray)
            .done();
    }

    #[method]
    fn add_env_variable(&mut self, key: String, value: String) {
        self.envs.insert(key, value);
    }

    #[method]
    fn get_env_variable(&self, key: String) -> Variant {
        self.envs
            .get(&key)
            .map_or_else(Variant::nil, |v| v.to_variant())
    }

    #[method]
    fn mount_physical_dir(&mut self, host_path: String, #[opt] guest_path: Option<String>) {
        self.physical_mount.insert(
            guest_path.unwrap_or_else(|| host_path.clone()).into(),
            host_path.into(),
        );
    }

    #[method]
    fn write_memory_file(&mut self, path: String, data: Variant, #[opt] offset: Option<usize>) {
        fn f(root: Arc<Dir>, path: &Path, data: &[u8], offset: Option<usize>) -> Result<(), Error> {
            let mut node: Arc<dyn Node> = root;
            for c in path.parent().unwrap_or(path).components() {
                let n = match c {
                    Component::CurDir => continue,
                    Component::ParentDir => node.parent(),
                    Component::RootDir => continue,
                    Component::Normal(name) => node.child(name.to_str().unwrap()),
                    Component::Prefix(_) => bail_with_site!("Windows-like paths is not supported"),
                };
                if let Some(n) = n {
                    node = n;
                } else if let Component::Normal(name) = c {
                    let Some(n) = node.as_any().downcast_ref::<Dir>() else { bail_with_site!("Cannot create directory") };
                    let n = n
                        .content
                        .write()
                        .entry(name.to_str().unwrap().to_owned())
                        .or_insert_with(|| Arc::new(Dir::new(Arc::downgrade(&node))))
                        .clone();
                    node = n;
                } else {
                    bail_with_site!("Path not found!");
                }
            }

            let Some(name) = path.file_name().and_then(|v| v.to_str()) else { return Ok(()) };
            let Some(n) = node.as_any().downcast_ref::<Dir>() else { bail_with_site!("Cannot create directory") };
            match n.content.write().entry(name.to_owned()) {
                Entry::Occupied(v) => {
                    let Some(file) = v.get().as_any().downcast_ref::<File>() else { bail_with_site!("Is a directory") };
                    let mut content = file.content.write();
                    if let Some(offset) = offset {
                        let mut cursor = Cursor::new(&mut *content);
                        cursor.set_position(offset as _);
                        cursor.write_all(data)?;
                    } else {
                        content.clear();
                        content.extend_from_slice(data);
                    }
                }
                Entry::Vacant(v) => {
                    let mut file = File::new(Arc::downgrade(&node));
                    *file.content.get_mut() = if let Some(offset) = offset {
                        let mut v = match offset.checked_add(data.len()) {
                            Some(v) => Vec::with_capacity(v),
                            None => bail_with_site!("Data too long!"),
                        };
                        v.resize(offset, 0);
                        v.extend_from_slice(data);
                        v
                    } else {
                        data.to_owned()
                    };
                    v.insert(Arc::new(file));
                }
            }

            Ok(())
        }

        unsafe fn as_bytes<T: Copy>(s: &[T]) -> &[u8] {
            slice::from_raw_parts(s.as_ptr() as *const u8, s.len() * mem::size_of::<T>())
        }

        Self::wrap_result(move || {
            let path = PathBuf::from(path);

            let f = |data| f(self.memfs_root.clone(), &path, data, offset);

            match data.dispatch() {
                VariantDispatch::ByteArray(v) => f(&*v.read()),
                VariantDispatch::GodotString(v) => f(v.to_string().as_bytes()),
                VariantDispatch::Int32Array(v) => unsafe { f(as_bytes(&v.read())) },
                VariantDispatch::Float32Array(v) => unsafe { f(as_bytes(&v.read())) },
                _ => bail_with_site!("Unknown value {}", data),
            }
        });
    }

    #[method]
    fn read_memory_file(
        &self,
        path: String,
        length: usize,
        #[opt] offset: usize,
    ) -> Option<PoolArray<u8>> {
        Self::wrap_result(move || {
            let path = PathBuf::from(path);

            let mut node: Arc<dyn Node> = self.memfs_root.clone();
            for c in path.parent().unwrap_or(&path).components() {
                let n = match c {
                    Component::CurDir => continue,
                    Component::ParentDir => node.parent(),
                    Component::RootDir => continue,
                    Component::Normal(name) => node.child(name.to_str().unwrap()),
                    Component::Prefix(_) => bail_with_site!("Windows-like paths is not supported"),
                };
                if let Some(n) = n {
                    node = n;
                } else {
                    bail_with_site!("Path not found!");
                }
            }

            let Some(name) = path.file_name().and_then(|v| v.to_str()) else { return Ok(PoolArray::new()) };
            let Some(n) = node.as_any().downcast_ref::<Dir>() else { bail_with_site!("Cannot create directory") };
            let content = n.content.read();
            let Some(file) = content.get(name).and_then(|v| v.as_any().downcast_ref::<File>()) else { bail_with_site!("File not found") };

            let content = file.content.read();
            let end = match offset.checked_add(length) {
                Some(v) => v.min(content.len()),
                None => content.len(),
            };
            let s = &content[offset.min(content.len())..end];
            Ok(PoolArray::from_slice(s))
        })
    }
}
