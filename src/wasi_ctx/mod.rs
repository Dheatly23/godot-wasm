pub mod memfs;
pub mod stdio;
pub mod timestamp;

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use anyhow::Error;
use camino::{Utf8Path, Utf8PathBuf};
use godot::prelude::*;
use wasi_common::dir::OpenResult;
use wasi_common::file::{FdFlags, FileType, OFlags};
use wasmtime_wasi::dir::{Dir as CapDir, OpenResult as OpenResult2};
use wasmtime_wasi::{ambient_authority, Dir as PhysicalDir, WasiCtx, WasiCtxBuilder};

use crate::wasi_ctx::memfs::{open, Capability, Dir, File, FileEntry, Link, Node};
use crate::wasi_ctx::stdio::{BlockWritePipe, LineWritePipe, UnbufferedWritePipe};
use crate::wasi_ctx::timestamp::{from_unix_time, to_unix_time};
use crate::wasm_config::{Config, PipeBindingType, PipeBufferType};
use crate::wasm_util::{
    option_to_variant, variant_to_option, FILE_DIR, FILE_FILE, FILE_LINK, FILE_NOTEXIST,
};
use crate::{bail_with_site, site_context};

#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct WasiContext {
    #[init(default = false)]
    #[export]
    bypass_stdio: bool,
    #[init(default = false)]
    #[export]
    fs_readonly: bool,

    #[init(default = Arc::new(Dir::new(<Weak<Dir>>::new())))]
    memfs_root: Arc<Dir>,
    #[init(default = HashMap::new())]
    physical_mount: HashMap<Utf8PathBuf, Utf8PathBuf>,
    #[init(default = HashMap::new())]
    envs: HashMap<String, String>,
}

impl WasiContext {
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
        this: Gd<Self>,
        mut ctx: WasiCtxBuilder,
        config: &Config,
    ) -> Result<WasiCtx, Error> {
        let o = this.bind();
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
        // TODO: Emit signal
        if config.wasi_stdout == PipeBindingType::Context {
            if o.bypass_stdio {
                ctx.inherit_stdout();
            } else {
                ctx.stdout(match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered => {
                        Box::new(UnbufferedWritePipe::new(move |_buf| {})) as _
                    }
                    PipeBufferType::LineBuffer => Box::new(LineWritePipe::new(move |_buf| {})) as _,
                    PipeBufferType::BlockBuffer => {
                        Box::new(BlockWritePipe::new(move |_buf| {})) as _
                    }
                });
            }
        }
        if config.wasi_stderr == PipeBindingType::Context {
            if o.bypass_stdio {
                ctx.inherit_stderr();
            } else {
                ctx.stderr(match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered => {
                        Box::new(UnbufferedWritePipe::new(move |_buf| {})) as _
                    }
                    PipeBufferType::LineBuffer => Box::new(LineWritePipe::new(move |_buf| {})) as _,
                    PipeBufferType::BlockBuffer => {
                        Box::new(BlockWritePipe::new(move |_buf| {})) as _
                    }
                });
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
            let dir = site_context!(PhysicalDir::open_ambient_dir(host, ambient_authority()))?;
            let OpenResult2::Dir(dir) = site_context!(CapDir::from_cap_std(dir).open_file_(
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
                write: !o.fs_readonly,
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
    }

    fn wrap_result<F, T>(f: F) -> Option<T>
    where
        F: FnOnce() -> Result<T, Error>,
    {
        match f() {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }
}

#[godot_api]
impl WasiContext {
    #[func]
    fn add_env_variable(&mut self, key: GodotString, value: GodotString) {
        self.envs.insert(key.to_string(), value.to_string());
    }

    #[func]
    fn get_env_variable(&self, key: GodotString) -> Variant {
        self.envs
            .get(&key.to_string())
            .map_or_else(Variant::nil, |v| v.to_variant())
    }

    #[func]
    fn delete_env_variable(&mut self, key: GodotString) -> Variant {
        option_to_variant(self.envs.remove(&key.to_string()).map(GodotString::from))
    }

    #[func]
    fn mount_physical_dir(&mut self, host_path: GodotString, guest_path: Variant) {
        let host_path = host_path.to_string();
        let guest_path = match variant_to_option(guest_path) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                return;
            }
        };
        self.physical_mount.insert(
            guest_path.unwrap_or_else(|| host_path.clone()).into(),
            host_path.into(),
        );
    }

    #[func]
    fn get_mounts(&self) -> Dictionary {
        self.physical_mount
            .iter()
            .map(|(k, v)| (GodotString::from(k), GodotString::from(v)))
            .collect()
    }

    #[func]
    fn unmount_physical_dir(&mut self, guest_path: GodotString) -> bool {
        self.physical_mount
            .remove(Utf8Path::new(&guest_path.to_string()))
            .is_some()
    }

    #[func]
    fn file_is_exist(&self, path: GodotString, follow_symlink: Variant) -> u32 {
        let Ok(follow_symlink) = variant_to_option(follow_symlink) else {
            return FILE_NOTEXIST;
        };
        match open(
            &path.to_string(),
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

    #[func]
    fn file_make_dir(&self, path: GodotString, name: GodotString, follow_symlink: Variant) -> bool {
        Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
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

    #[func]
    fn file_make_file(
        &self,
        path: GodotString,
        name: GodotString,
        follow_symlink: Variant,
    ) -> bool {
        Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
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

    #[func]
    fn file_make_link(
        &self,
        path: GodotString,
        name: GodotString,
        link: GodotString,
        follow_symlink: Variant,
    ) -> bool {
        Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
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

    #[func]
    fn file_delete_file(
        &self,
        path: GodotString,
        name: GodotString,
        follow_symlink: Variant,
    ) -> bool {
        Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
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

    #[func]
    fn file_dir_list(&self, path: GodotString, follow_symlink: Variant) -> PackedStringArray {
        Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
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

            let ret = dir.content.read().keys().map(GodotString::from).collect();
            Ok(ret)
        })
        .unwrap_or_else(PackedStringArray::new)
    }

    #[func]
    fn file_stat(&self, path: GodotString, follow_symlink: Variant) -> Variant {
        option_to_variant(Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
                self.memfs_root.clone(),
                &Some(self.memfs_root.clone()),
                follow_symlink.unwrap_or(false),
                false,
            ) else {
                bail_with_site!("Failed to open path {path}")
            };

            let stat = f.filestat();
            let mut dict = Dictionary::new();
            dict.insert(
                "filetype",
                match stat.filetype {
                    FileType::Directory => FILE_DIR,
                    FileType::RegularFile => FILE_FILE,
                    FileType::SymbolicLink => FILE_LINK,
                    _ => FILE_NOTEXIST,
                },
            );
            dict.insert("size", stat.size as i64);

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

            Ok(dict)
        }))
    }

    #[func]
    fn file_set_time(&self, path: GodotString, time: Dictionary, follow_symlink: Variant) -> bool {
        Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let mtime = time
                .get("mtime")
                .and_then(|v| variant_to_option::<i64>(v).transpose())
                .transpose()?;
            let atime = time
                .get("atime")
                .and_then(|v| variant_to_option::<i64>(v).transpose())
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
                &path.to_string(),
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

    #[func]
    fn file_link_target(&self, path: GodotString, follow_symlink: Variant) -> Variant {
        option_to_variant(Self::wrap_result(move || {
            let follow_symlink = variant_to_option(follow_symlink)?;
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
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

            Ok(GodotString::from(&link.path))
        }))
    }

    #[func]
    fn file_read(
        &self,
        path: GodotString,
        length: i64,
        offset: Variant,
        follow_symlink: Variant,
    ) -> Variant {
        option_to_variant(Self::wrap_result(move || {
            let length = length as usize;
            let offset = variant_to_option::<i64>(offset)?.map(|v| v as usize);
            let follow_symlink = variant_to_option(follow_symlink)?;
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
                &path.to_string(),
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
                Ok(PackedByteArray::from(s))
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
        }))
    }

    #[func]
    fn file_write(
        &self,
        path: GodotString,
        data: Variant,
        offset: Variant,
        truncate: Variant,
        follow_symlink: Variant,
    ) -> bool {
        fn f<R>(
            root: Arc<Dir>,
            path: GodotString,
            follow_symlink: bool,
            truncate: bool,
            offset: usize,
            end: usize,
            f_: impl FnOnce(&mut [u8]) -> Result<R, Error>,
        ) -> Result<R, Error> {
            let Ok(FileEntry::Occupied(f)) = open(
                &path.to_string(),
                root.clone(),
                &Some(root),
                follow_symlink,
                false,
            ) else {
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
            path: GodotString,
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

        Self::wrap_result(move || {
            let offset = variant_to_option::<i64>(offset)?.map(|v| v as usize);
            let truncate = variant_to_option(truncate)?.unwrap_or(false);
            let follow_symlink = variant_to_option(follow_symlink)?.unwrap_or(false);
            if let Ok(s) = PackedByteArray::try_from_variant(&data) {
                let s = s.as_slice();
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
            } else if let Ok(s) = GodotString::try_from_variant(&data) {
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
            } else if let Ok(s) = PackedInt32Array::try_from_variant(&data) {
                g::<4, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| *d = s.to_le_bytes(),
                )
            } else if let Ok(s) = PackedInt64Array::try_from_variant(&data) {
                g::<8, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| *d = s.to_le_bytes(),
                )
            } else if let Ok(s) = PackedFloat32Array::try_from_variant(&data) {
                g::<4, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| *d = s.to_le_bytes(),
                )
            } else if let Ok(s) = PackedFloat64Array::try_from_variant(&data) {
                g::<8, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| *d = s.to_le_bytes(),
                )
            } else if let Ok(s) = PackedVector2Array::try_from_variant(&data) {
                g::<8, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| {
                        d[..4].copy_from_slice(&s.x.to_le_bytes());
                        d[4..].copy_from_slice(&s.y.to_le_bytes());
                    },
                )
            } else if let Ok(s) = PackedVector3Array::try_from_variant(&data) {
                g::<12, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| {
                        d[..4].copy_from_slice(&s.x.to_le_bytes());
                        d[4..8].copy_from_slice(&s.y.to_le_bytes());
                        d[8..].copy_from_slice(&s.z.to_le_bytes());
                    },
                )
            } else if let Ok(s) = PackedColorArray::try_from_variant(&data) {
                g::<16, _>(
                    self.memfs_root.clone(),
                    path,
                    follow_symlink,
                    truncate,
                    offset,
                    s.as_slice(),
                    |s, d| {
                        d[..4].copy_from_slice(&s.r.to_le_bytes());
                        d[4..8].copy_from_slice(&s.g.to_le_bytes());
                        d[8..12].copy_from_slice(&s.b.to_le_bytes());
                        d[12..].copy_from_slice(&s.a.to_le_bytes());
                    },
                )
            } else {
                bail_with_site!("Unknown value type {:?}", data.get_type())
            }
        })
        .is_some()
    }
}
