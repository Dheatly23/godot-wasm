pub mod memfs;
pub mod stdio;
pub mod timestamp;

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use anyhow::Error;
use camino::{Utf8Path, Utf8PathBuf};
use gdnative::log::{error, godot_site, Site};
use gdnative::prelude::*;
use wasi_common::dir::OpenResult;
use wasi_common::file::{FdFlags, FileType, OFlags};
use wasi_common::WasiCtx;
use wasmtime_wasi::dir::{Dir as CapDir, OpenResult as OpenResult2};
#[cfg(feature = "wasi-preview2")]
use wasmtime_wasi::preview2::{
    DirPerms, FilePerms, WasiCtx as WasiCtxPv2, WasiCtxBuilder as WasiCtxBuilderPv2,
};
use wasmtime_wasi::{ambient_authority, Dir as PhysicalDir, WasiCtxBuilder};

use crate::wasi_ctx::memfs::{open, Capability, Dir, File, FileEntry, Link, Node};
#[cfg(feature = "wasi-preview2")]
use crate::wasi_ctx::stdio::StreamWrapper;
use crate::wasi_ctx::stdio::{BlockWritePipe, LineWritePipe, UnbufferedWritePipe};
use crate::wasi_ctx::timestamp::{from_unix_time, to_unix_time};
use crate::wasm_config::{Config, PipeBindingType, PipeBufferType};
use crate::wasm_util::{FILE_DIR, FILE_FILE, FILE_LINK, FILE_NOTEXIST};
use crate::{bail_with_site, site_context};

#[derive(NativeClass, Debug)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::RwLockData<WasiContext>)]
pub struct WasiContext {
    bypass_stdio: bool,
    fs_readonly: bool,

    memfs_root: Arc<Dir>,
    physical_mount: HashMap<Utf8PathBuf, Utf8PathBuf>,
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

    fn emit_binary(
        base: Ref<Reference>,
        signal_name: &'static str,
    ) -> impl Fn(&[u8]) + Send + Sync + Clone + 'static {
        move |buf| unsafe {
            base.assume_safe().emit_signal(
                signal_name,
                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
            );
        }
    }

    fn emit_string(
        base: Ref<Reference>,
        signal_name: &'static str,
    ) -> impl Fn(&[u8]) + Send + Sync + Clone + 'static {
        move |buf| unsafe {
            base.assume_safe()
                .emit_signal(signal_name, &[String::from_utf8_lossy(buf).to_variant()]);
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
        mut ctx: WasiCtxBuilder,
        config: &Config,
    ) -> Result<WasiCtx, Error> {
        let f = move |o: &Self, b: TRef<'_, Reference>| -> Result<_, Error> {
            if let (PipeBindingType::Context, Some(file)) =
                (&config.wasi_stdin, &config.wasi_stdin_file)
            {
                let root = Some(o.memfs_root.clone());
                let node = if let FileEntry::Occupied(v) = site_context!(open(
                    file,
                    o.memfs_root.clone(),
                    &Some(o.memfs_root.clone()),
                    true,
                    false,
                ))? {
                    v.into_inner()
                } else {
                    bail_with_site!("Path \"{}\" not found!", file)
                };

                let OpenResult::File(file) = site_context!(node.open(
                    root,
                    Capability {
                        read: true,
                        write: false
                    },
                    true,
                    OFlags::empty(),
                    FdFlags::empty(),
                ))?
                else {
                    bail_with_site!("Path \"{}\" should be a file!", file)
                };
                ctx.stdin(file);
            }
            if config.wasi_stdout == PipeBindingType::Context {
                if o.bypass_stdio {
                    ctx.inherit_stdout();
                } else {
                    let base = b.claim();
                    match config.wasi_stdout_buffer {
                        PipeBufferType::Unbuffered => ctx.stdout(Box::new(
                            UnbufferedWritePipe::new(Self::emit_binary(base, "stdout_emit")),
                        )),
                        PipeBufferType::LineBuffer => ctx.stdout(Box::new(LineWritePipe::new(
                            Self::emit_string(base, "stdout_emit"),
                        ))),
                        PipeBufferType::BlockBuffer => ctx.stdout(Box::new(BlockWritePipe::new(
                            Self::emit_binary(base, "stdout_emit"),
                        ))),
                    };
                }
            }
            if config.wasi_stderr == PipeBindingType::Context {
                if o.bypass_stdio {
                    ctx.inherit_stderr();
                } else {
                    let base = b.claim();
                    match config.wasi_stderr_buffer {
                        PipeBufferType::Unbuffered => ctx.stderr(Box::new(
                            UnbufferedWritePipe::new(Self::emit_binary(base, "stderr_emit")),
                        )),
                        PipeBufferType::LineBuffer => ctx.stderr(Box::new(LineWritePipe::new(
                            Self::emit_string(base, "stderr_emit"),
                        ))),
                        PipeBufferType::BlockBuffer => ctx.stderr(Box::new(BlockWritePipe::new(
                            Self::emit_binary(base, "stderr_emit"),
                        ))),
                    };
                }
            }

            let c = ctx.build();
            drop(ctx);
            let mut ctx = Self::init_ctx_no_context(c, config)?;

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
                ))?
                else {
                    bail_with_site!("Path should be a directory!")
                };
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
            ))?
            else {
                bail_with_site!("Root should be a directory!")
            };
            site_context!(ctx.push_preopened_dir(root, "."))?;

            Ok(ctx)
        };

        unsafe { this.assume_safe().map(f)? }
    }

    #[cfg(feature = "wasi-preview2")]
    pub fn init_ctx_no_context_preview_2(
        ctx: &mut WasiCtxBuilderPv2,
        config: &Config,
    ) -> Result<(), Error> {
        for (k, v) in &config.wasi_envs {
            ctx.env(k, v);
        }

        ctx.args(&config.wasi_args);

        Ok(())
    }

    #[cfg(feature = "wasi-preview2")]
    pub fn build_ctx_preview_2(
        this: Instance<Self>,
        mut ctx: WasiCtxBuilderPv2,
        config: &Config,
    ) -> Result<WasiCtxPv2, Error> {
        let f = move |o: &Self, b: TRef<'_, Reference>| -> Result<_, Error> {
            if config.wasi_stdout == PipeBindingType::Context {
                if o.bypass_stdio {
                    ctx.inherit_stdout();
                } else {
                    let base = b.claim();
                    match config.wasi_stdout_buffer {
                        PipeBufferType::Unbuffered => ctx.stdout(UnbufferedWritePipe::new(
                            Self::emit_binary(base, "stdout_emit"),
                        )),
                        PipeBufferType::LineBuffer => ctx.stdout(StreamWrapper::from(
                            LineWritePipe::new(Self::emit_string(base, "stdout_emit")),
                        )),
                        PipeBufferType::BlockBuffer => ctx.stdout(StreamWrapper::from(
                            BlockWritePipe::new(Self::emit_binary(base, "stdout_emit")),
                        )),
                    };
                }
            }
            if config.wasi_stderr == PipeBindingType::Context {
                if o.bypass_stdio {
                    ctx.inherit_stderr();
                } else {
                    let base = b.claim();
                    match config.wasi_stderr_buffer {
                        PipeBufferType::Unbuffered => ctx.stderr(UnbufferedWritePipe::new(
                            Self::emit_binary(base, "stderr_emit"),
                        )),
                        PipeBufferType::LineBuffer => ctx.stderr(StreamWrapper::from(
                            LineWritePipe::new(Self::emit_string(base, "stderr_emit")),
                        )),
                        PipeBufferType::BlockBuffer => ctx.stderr(StreamWrapper::from(
                            BlockWritePipe::new(Self::emit_binary(base, "stderr_emit")),
                        )),
                    };
                }
            }

            Self::init_ctx_no_context_preview_2(&mut ctx, config)?;

            for (k, v) in o
                .envs
                .iter()
                .filter(|(k, _)| !config.wasi_envs.contains_key(&**k))
            {
                ctx.env(k, v);
            }

            let fs_writable = !(o.fs_readonly || config.wasi_fs_readonly);
            let (perms, file_perms) = if fs_writable {
                (
                    DirPerms::READ | DirPerms::MUTATE,
                    FilePerms::READ | FilePerms::WRITE,
                )
            } else {
                (DirPerms::READ, FilePerms::READ)
            };

            for (guest, host) in o.physical_mount.iter() {
                let dir = site_context!(PhysicalDir::open_ambient_dir(host, ambient_authority(),))?;
                ctx.preopened_dir(dir, perms, file_perms, guest);
            }

            // XXX: Cannot do memory filesystem yet :((
            /*
            let OpenResult::Dir(root) = site_context!(o.memfs_root.clone().open(
                Some(o.memfs_root.clone()),
                Capability {
                    read: true,
                    write: fs_writable,
                },
                true,
                OFlags::DIRECTORY,
                FdFlags::empty(),
            ))?
            else {
                bail_with_site!("Root should be a directory!")
            };
            site_context!(ctx.push_preopened_dir(root, "."))?;
            */

            Ok(ctx.build())
        };

        unsafe { this.assume_safe().map(f)? }
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
            .with_param("message", VariantType::GodotString)
            .done();

        builder
            .signal("stderr_emit")
            .with_param("message", VariantType::GodotString)
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
    fn delete_env_variable(&mut self, key: String) -> Option<GodotString> {
        self.envs.remove(&key).map(GodotString::from)
    }

    #[method]
    fn mount_physical_dir(&mut self, host_path: String, #[opt] guest_path: Option<String>) {
        self.physical_mount.insert(
            guest_path.unwrap_or_else(|| host_path.clone()).into(),
            host_path.into(),
        );
    }

    #[method]
    fn get_mounts(&self) -> Dictionary<Unique> {
        self.physical_mount
            .iter()
            .map(|(k, v)| (GodotString::from_str(k), GodotString::from_str(v)))
            .collect()
    }

    #[method]
    fn unmount_physical_dir(&mut self, guest_path: String) -> bool {
        self.physical_mount
            .remove(Utf8Path::new(&guest_path))
            .is_some()
    }

    #[method]
    fn file_is_exist(&self, path: String, #[opt] follow_symlink: Option<bool>) -> u32 {
        match open(
            &path,
            self.memfs_root.clone(),
            &Some(self.memfs_root.clone()),
            follow_symlink.unwrap_or(false),
            false,
        ) {
            Ok(FileEntry::Occupied(f)) => match f.filetype() {
                FileType::Directory => FILE_DIR,
                FileType::RegularFile => FILE_FILE,
                FileType::SymbolicLink => FILE_LINK,
                _ => FILE_NOTEXIST,
            },
            _ => FILE_NOTEXIST,
        }
    }

    #[method]
    fn file_make_dir(
        &self,
        path: String,
        name: GodotString,
        #[opt] follow_symlink: Option<bool>,
    ) -> bool {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(dir) = f.as_any().downcast_ref::<Dir>() else {
                bail_with_site!("Path {path} is not a directory")
            };
            let mut ret = false;
            dir.content
                .write()
                .entry(name.to_string())
                .or_insert_with(|| {
                    ret = true;
                    Arc::new(Dir::new(Arc::downgrade(&*f)))
                });

            Ok(ret)
        })
        .unwrap_or(false)
    }

    #[method]
    fn file_make_file(
        &self,
        path: String,
        name: GodotString,
        #[opt] follow_symlink: Option<bool>,
    ) -> bool {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(dir) = f.as_any().downcast_ref::<Dir>() else {
                bail_with_site!("Path {path} is not a directory")
            };
            let mut ret = false;
            dir.content
                .write()
                .entry(name.to_string())
                .or_insert_with(|| {
                    ret = true;
                    Arc::new(File::new(Arc::downgrade(&*f)))
                });

            Ok(ret)
        })
        .unwrap_or(false)
    }

    #[method]
    fn file_make_link(
        &self,
        path: String,
        name: GodotString,
        link: GodotString,
        #[opt] follow_symlink: Option<bool>,
    ) -> bool {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(dir) = f.as_any().downcast_ref::<Dir>() else {
                bail_with_site!("Path {path} is not a directory")
            };
            let mut ret = false;
            dir.content
                .write()
                .entry(name.to_string())
                .or_insert_with(|| {
                    ret = true;
                    Arc::new(Link::new(Arc::downgrade(&*f), link.to_string().into()))
                });

            Ok(ret)
        })
        .unwrap_or(false)
    }

    #[method]
    fn file_delete_file(
        &self,
        path: String,
        name: GodotString,
        #[opt] follow_symlink: Option<bool>,
    ) -> bool {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(dir) = f.as_any().downcast_ref::<Dir>() else {
                bail_with_site!("Path {path} is not a directory")
            };

            if dir.content.write().remove(&name.to_string()).is_none() {
                bail_with_site!("File {name} does not exists in {path}");
            }

            Ok(())
        })
        .is_some()
    }

    #[method]
    fn file_dir_list(
        &self,
        path: String,
        #[opt] follow_symlink: Option<bool>,
    ) -> Option<PoolArray<GodotString>> {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(dir) = f.as_any().downcast_ref::<Dir>() else {
                bail_with_site!("Path {path} is not a directory")
            };

            let ret = dir
                .content
                .read()
                .keys()
                .map(GodotString::from_str)
                .collect();
            Ok(ret)
        })
    }

    #[method]
    fn file_stat(&self, path: String, #[opt] follow_symlink: Option<bool>) -> Option<Dictionary> {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };

            let stat = f.filestat();
            let dict = Dictionary::new();
            dict.insert(
                "filetype",
                match stat.filetype {
                    FileType::Directory => FILE_DIR,
                    FileType::RegularFile => FILE_FILE,
                    FileType::SymbolicLink => FILE_LINK,
                    _ => FILE_NOTEXIST,
                },
            );
            dict.insert("size", stat.size);

            fn g(time: SystemTime) -> i64 {
                let v = to_unix_time(time);
                match i64::try_from(v) {
                    Ok(v) => v,
                    Err(_) if v >= 0 => i64::MAX,
                    Err(_) => i64::MIN,
                }
            }

            dict.insert("atime", stat.atim.map_or(0, g));
            dict.insert("mtime", stat.mtim.map_or(0, g));
            dict.insert("ctime", stat.ctim.map_or(0, g));

            Ok(dict.into_shared())
        })
    }

    #[method]
    fn file_set_time(
        &self,
        path: String,
        time: Dictionary,
        #[opt] follow_symlink: Option<bool>,
    ) -> bool {
        Self::wrap_result(move || {
            let mtime = time
                .get("mtime")
                .and_then(|v| <Option<i64>>::from_variant(&v).transpose())
                .transpose()?;
            let atime = time
                .get("atime")
                .and_then(|v| <Option<i64>>::from_variant(&v).transpose())
                .transpose()?;

            let (mtime, atime) = match (
                mtime.and_then(from_unix_time),
                atime.and_then(from_unix_time),
            ) {
                (None, None) => {
                    let t = Some(SystemTime::now());
                    (t, t)
                }
                (t @ Some(_), None) => (t, t),
                t @ (_, Some(_)) => t,
            };

            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };

            let stamp = f.timestamp();
            if let Some(mtime) = mtime {
                stamp.mtime.set_stamp(mtime);
            }
            if let Some(atime) = atime {
                stamp.atime.set_stamp(atime);
            }

            Ok(())
        })
        .is_some()
    }

    #[method]
    fn file_link_target(
        &self,
        path: String,
        #[opt] follow_symlink: Option<bool>,
    ) -> Option<GodotString> {
        Self::wrap_result(move || {
            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(link) = f.as_any().downcast_ref::<Link>() else {
                bail_with_site!("Path {path} is not a symlink")
            };

            Ok(GodotString::from_str(&link.path))
        })
    }

    #[method]
    fn file_read(
        &self,
        path: String,
        length: usize,
        #[opt] offset: Option<usize>,
        #[opt] follow_symlink: Option<bool>,
    ) -> Option<PoolArray<u8>> {
        Self::wrap_result(move || {
            let offset = offset.unwrap_or(0);
            let end = if length > 0 {
                match offset.checked_add(length) {
                    None => bail_with_site!("Length overflowed"),
                    v => v,
                }
            } else {
                None
            };

            let Ok(FileEntry::Occupied(f)) = open(
                &path,
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(f) = f.as_any().downcast_ref::<File>() else {
                bail_with_site!("Path {path} is not a file")
            };

            let guard = f.content.read();
            let mut s = None;
            if let Some(end) = end {
                s = guard.get(offset..end);
            }
            if s.is_none() {
                s = guard.get(offset..);
            }
            if let Some(s) = s {
                Ok(PoolArray::from_slice(s))
            } else if let Some(end) = end {
                bail_with_site!(
                    "Index {}..{} overflowed (file size is {})",
                    offset,
                    end,
                    guard.len()
                )
            } else {
                bail_with_site!(
                    "Index {}.. overflowed (file size is {})",
                    offset,
                    guard.len()
                )
            }
        })
    }

    #[method]
    fn file_write(
        &self,
        path: String,
        data: Variant,
        #[opt] offset: Option<usize>,
        #[opt] truncate: Option<bool>,
        #[opt] follow_symlink: Option<bool>,
    ) -> bool {
        fn f<R>(
            root: Arc<Dir>,
            path: String,
            follow_symlink: bool,
            truncate: bool,
            offset: usize,
            end: usize,
            f_: impl FnOnce(&mut [u8]) -> Result<R, Error>,
        ) -> Result<R, Error> {
            let Ok(FileEntry::Occupied(f)) =
                open(&path, root.clone(), &Some(root), follow_symlink, false)
            else {
                bail_with_site!("Failed to open path {path}")
            };
            let Some(f) = f.as_any().downcast_ref::<File>() else {
                bail_with_site!("Path {path} is not a file")
            };

            let mut guard = f.content.write();
            if truncate || guard.len() < end {
                guard.resize(end, 0);
            }

            f_(&mut guard[offset..end])
        }

        fn g<const N: usize, T>(
            root: Arc<Dir>,
            path: String,
            follow_symlink: bool,
            truncate: bool,
            offset: Option<usize>,
            data: &[T],
            f_: impl Fn(&T, &mut [u8; N]),
        ) -> Result<(), Error> {
            let offset = offset.unwrap_or(0);
            let Some(end) = data
                .len()
                .checked_mul(N)
                .and_then(|v| v.checked_add(offset))
            else {
                bail_with_site!("Length overflowed")
            };

            let f_ = move |s: &mut [u8]| {
                for (s, d) in data.iter().zip(s.chunks_mut(N)) {
                    f_(s, d.try_into().unwrap())
                }

                Ok(())
            };

            f(root, path, follow_symlink, truncate, offset, end, f_)
        }

        let follow_symlink = follow_symlink.unwrap_or(false);
        let truncate = truncate.unwrap_or(false);

        Self::wrap_result(move || match data.dispatch() {
            VariantDispatch::ByteArray(s) => {
                let s = s.read();
                let offset = offset.unwrap_or(0);
                let Some(end) = s.len().checked_add(offset) else {
                    bail_with_site!("Length overflowed")
                };

                f(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    end,
                    move |d| {
                        d.copy_from_slice(&s);
                        Ok(())
                    },
                )
            }
            VariantDispatch::GodotString(s) => {
                let s = s.to_string();
                let s = s.as_bytes();
                let offset = offset.unwrap_or(0);
                let Some(end) = s.len().checked_add(offset) else {
                    bail_with_site!("Length overflowed")
                };

                f(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    end,
                    move |d| {
                        d.copy_from_slice(s);
                        Ok(())
                    },
                )
            }
            VariantDispatch::Int32Array(s) => g::<4, _>(
                self.memfs_root.clone(),
                path,
                follow_symlink,
                truncate,
                offset,
                &s.read(),
                |s, d| *d = s.to_le_bytes(),
            ),
            VariantDispatch::Float32Array(s) => g::<4, _>(
                self.memfs_root.clone(),
                path,
                follow_symlink,
                truncate,
                offset,
                &s.read(),
                |s, d| *d = s.to_le_bytes(),
            ),
            VariantDispatch::Vector2Array(s) => g::<8, _>(
                self.memfs_root.clone(),
                path,
                follow_symlink,
                truncate,
                offset,
                &s.read(),
                |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[4..]).unwrap() = s.y.to_le_bytes();
                },
            ),
            VariantDispatch::Vector3Array(s) => g::<12, _>(
                self.memfs_root.clone(),
                path,
                follow_symlink,
                truncate,
                offset,
                &s.read(),
                |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[4..8]).unwrap() = s.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[8..]).unwrap() = s.z.to_le_bytes();
                },
            ),
            VariantDispatch::ColorArray(s) => g::<16, _>(
                self.memfs_root.clone(),
                path,
                follow_symlink,
                truncate,
                offset,
                &s.read(),
                |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.r.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[4..8]).unwrap() = s.g.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[8..12]).unwrap() = s.b.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[12..]).unwrap() = s.a.to_le_bytes();
                },
            ),
            _ => bail_with_site!("Unknown value type {:?}", data.get_type()),
        })
        .is_some()
    }
}
