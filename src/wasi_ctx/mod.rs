pub mod stdio;

use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result as AnyResult;
use camino::{Utf8Path, Utf8PathBuf};

use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, MutexGuard};
use wasi_isolated_fs::context::WasiContextBuilder;
use wasi_isolated_fs::fs_isolated::{
    AccessMode, CapWrapper, CreateParams, Dir, File, IsolatedFSController, Link, Node,
};

use crate::godot_util::{
    from_var_any, option_to_variant, variant_to_option, PhantomProperty, SendSyncWrapper,
    StructPacking,
};
use crate::wasm_config::{Config, PipeBindingType, PipeBufferType};
use crate::wasm_util::{FILE_DIR, FILE_FILE, FILE_LINK, FILE_NOTEXIST};
use crate::{bail_with_site, site_context, variant_dispatch};

fn to_unix_time(time: SystemTime) -> i128 {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i128::from(d.as_secs()),
        Err(d) => {
            let d = d.duration();
            let mut r = -i128::from(d.as_secs());
            if d.subsec_nanos() > 0 {
                r = r.saturating_sub(1);
            }
            r
        }
    }
}

fn from_unix_time(time: i64) -> Option<SystemTime> {
    if time >= 0 {
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(time as _))
    } else {
        SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(time.wrapping_neg() as _))
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted, init, tool)]
pub struct WasiContext {
    base: Base<RefCounted>,
    data: OnceCell<Mutex<WasiContextInner>>,

    /// Flag to pass through stdio into terminal.
    #[var(get = is_bypass_stdio, set = set_bypass_stdio)]
    #[allow(dead_code)]
    bypass_stdio: PhantomProperty<bool>,

    /// Flag to force filesystem to be read-only.
    #[var(get = is_fs_readonly, set = set_fs_readonly)]
    #[allow(dead_code)]
    fs_readonly: PhantomProperty<bool>,
}

struct WasiContextInner {
    bypass_stdio: bool,
    fs_readonly: bool,

    memfs_controller: IsolatedFSController,
    physical_mount: HashMap<Utf8PathBuf, Utf8PathBuf>,
    envs: HashMap<String, String>,
}

impl WasiContext {
    fn get_data(&self) -> AnyResult<MutexGuard<'_, WasiContextInner>> {
        if let Some(data) = self.data.get() {
            Ok(data.lock())
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

    fn wrap_data<T>(&self, f: impl FnOnce(&mut WasiContextInner) -> AnyResult<T>) -> Option<T> {
        match self.get_data().and_then(|mut v| f(&mut v)) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    pub fn emit_binary(signal: Signal) -> impl Fn(&[u8]) + Send + Sync + Clone + 'static {
        let signal = SendSyncWrapper::new(signal);
        move |buf| signal.emit(&[PackedByteArray::from(buf).to_variant()])
    }

    pub fn emit_string(signal: Signal) -> impl Fn(&str) + Send + Sync + Clone + 'static {
        let signal = SendSyncWrapper::new(signal);
        move |buf| signal.emit(&[buf.to_variant()])
    }

    pub fn init_ctx_no_context(ctx: &mut WasiContextBuilder, config: &Config) -> AnyResult<()> {
        ctx.envs(config.wasi_envs.iter().map(|(k, v)| (k.clone(), v.clone())))
            .args(config.wasi_args.iter().cloned());
        Ok(())
    }

    pub fn build_ctx(
        this: &Gd<Self>,
        ctx: &mut WasiContextBuilder,
        config: &Config,
    ) -> AnyResult<()> {
        let o = this.bind();
        let o = o.get_data()?;

        if config.wasi_stdout == PipeBindingType::Context {
            if o.bypass_stdio {
                ctx.stdout_bypass()
            } else {
                let signal = Signal::from_object_signal(this, c"stdout_emit");
                match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered | PipeBufferType::BlockBuffer => {
                        ctx.stdout_block_buffer(Box::new(WasiContext::emit_binary(signal)))
                    }
                    PipeBufferType::LineBuffer => {
                        ctx.stdout_line_buffer(Box::new(WasiContext::emit_string(signal)))
                    }
                }
            }?;
        }
        if config.wasi_stderr == PipeBindingType::Context {
            if o.bypass_stdio {
                ctx.stderr_bypass()
            } else {
                let signal = Signal::from_object_signal(this, c"stderr_emit");
                match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered | PipeBufferType::BlockBuffer => {
                        ctx.stderr_block_buffer(Box::new(WasiContext::emit_binary(signal)))
                    }
                    PipeBufferType::LineBuffer => {
                        ctx.stderr_line_buffer(Box::new(WasiContext::emit_string(signal)))
                    }
                }
            }?;
        }

        Self::init_ctx_no_context(&mut *ctx, config)?;

        ctx.envs(
            o.envs
                .iter()
                .filter_map(|(k, v)| match config.wasi_envs.contains_key(k) {
                    true => None,
                    false => Some((k.clone(), v.clone())),
                }),
        )
        .fs_readonly(o.fs_readonly || config.wasi_fs_readonly);

        site_context!(ctx.isolated_fs_controller(&o.memfs_controller))?;
        site_context!(ctx.preopen_dir_isolated("/".parse().unwrap(), "/".parse().unwrap()))?;

        for (guest, host) in o.physical_mount.iter() {
            site_context!(ctx.preopen_dir_host(host.clone(), guest.clone()))?;
        }

        Ok(())
    }
}

#[godot_api]
impl WasiContext {
    #[signal]
    fn stdout_emit(message: Variant);
    #[signal]
    fn stderr_emit(message: Variant);

    #[func]
    fn initialize(&self, config: Variant) -> Option<Gd<WasiContext>> {
        let r = self.data.get_or_try_init(move || -> AnyResult<_> {
            let config = site_context!(variant_to_option::<Dictionary>(config))?;

            Ok(Mutex::new(WasiContextInner {
                memfs_controller: site_context!(IsolatedFSController::new(
                    site_context!(config
                        .as_ref()
                        .and_then(|c| c.get("memfs.max_size"))
                        .map(from_var_any::<i64>)
                        .transpose())?
                    .map_or(isize::MAX as usize, |v| v as usize),
                    site_context!(config
                        .as_ref()
                        .and_then(|c| c.get("memfs.max_node"))
                        .map(from_var_any::<i64>)
                        .transpose())?
                    .map_or(isize::MAX as usize, |v| v as usize),
                ))?,
                physical_mount: HashMap::new(),
                envs: HashMap::new(),

                bypass_stdio: false,
                fs_readonly: false,
            }))
        });

        if let Err(e) = r {
            godot_error!("{e:?}");
            None
        } else {
            Some(self.to_gd())
        }
    }

    #[func]
    fn is_bypass_stdio(&self) -> bool {
        self.wrap_data(|v| Ok(v.bypass_stdio)).unwrap_or_default()
    }

    #[func]
    fn set_bypass_stdio(&self, v: bool) {
        self.wrap_data(|this| {
            this.bypass_stdio = v;
            Ok(())
        });
    }

    #[func]
    fn is_fs_readonly(&self) -> bool {
        self.wrap_data(|v| Ok(v.fs_readonly)).unwrap_or_default()
    }

    #[func]
    fn set_fs_readonly(&self, v: bool) {
        self.wrap_data(move |this| {
            this.fs_readonly = v;
            Ok(())
        });
    }

    #[func]
    fn add_env_variable(&self, key: GString, value: GString) {
        self.wrap_data(move |this| {
            this.envs.insert(key.to_string(), value.to_string());
            Ok(())
        });
    }

    #[func]
    fn get_env_variable(&self, key: GString) -> Variant {
        option_to_variant(
            self.wrap_data(move |this| Ok(this.envs.get(&key.to_string()).map(GString::from)))
                .flatten(),
        )
    }

    #[func]
    fn delete_env_variable(&self, key: GString) -> Variant {
        option_to_variant(
            self.wrap_data(move |this| Ok(this.envs.remove(&key.to_string()).map(GString::from)))
                .flatten(),
        )
    }

    #[func]
    fn mount_physical_dir(&self, host_path: GString, guest_path: Variant) {
        self.wrap_data(move |this| {
            let host_path = host_path.to_string();
            let guest_path =
                site_context!(variant_to_option(guest_path))?.unwrap_or_else(|| host_path.clone());
            this.physical_mount
                .insert(guest_path.into(), host_path.into());
            Ok(())
        });
    }

    #[func]
    fn get_mounts(&self) -> Variant {
        option_to_variant(self.wrap_data(|this| {
            Ok(this
                .physical_mount
                .iter()
                .map(|(k, v)| (GString::from(k.as_str()), GString::from(v.as_str())))
                .collect::<Dictionary>())
        }))
    }

    #[func]
    fn unmount_physical_dir(&mut self, guest_path: GString) -> Variant {
        option_to_variant(self.wrap_data(|this| {
            Ok(this
                .physical_mount
                .remove(Utf8Path::new(&guest_path.to_string()))
                .is_some())
        }))
    }

    #[func]
    fn file_is_exist(&self, path: GString, follow_symlink: Variant) -> Variant {
        option_to_variant(self.wrap_data(move |this| {
            match CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                &this.memfs_controller,
                &Utf8PathBuf::from(path.to_string()),
                site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                None,
                AccessMode::RW,
            ) {
                Ok(f) => {
                    let n = &**f.node();
                    Ok(if n.is_link() {
                        FILE_LINK
                    } else if n.is_dir() {
                        FILE_DIR
                    } else {
                        FILE_FILE
                    })
                }
                Err(e) if e.io().map(|e| e.kind()) == Some(ErrorKind::NotFound) => {
                    Ok(FILE_NOTEXIST)
                }
                Err(e) => site_context!(Err(e)),
            }
        }))
    }

    #[func]
    fn file_make_dir(&self, path: GString, name: GString, follow_symlink: Variant) -> bool {
        self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let mut n = site_context!(f.node().try_dir())?;
            site_context!(n.add(name.to_string(), || -> AnyResult<_> {
                Ok(Arc::new(Node::from((
                    Dir::new(&this.memfs_controller)?,
                    Arc::downgrade(f.node()),
                ))))
            }))
            .map(|v| v.is_some())
        })
        .unwrap_or_default()
    }

    #[func]
    fn file_make_file(&self, path: GString, name: GString, follow_symlink: Variant) -> bool {
        self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let mut n = site_context!(f.node().try_dir())?;
            site_context!(n.add(name.to_string(), || -> AnyResult<_> {
                Ok(Arc::new(Node::from((
                    File::new(&this.memfs_controller)?,
                    Arc::downgrade(f.node()),
                ))))
            }))
            .map(|v| v.is_some())
        })
        .unwrap_or_default()
    }

    #[func]
    fn file_make_link(
        &self,
        path: GString,
        name: GString,
        link: GString,
        follow_symlink: Variant,
    ) -> bool {
        self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let mut n = site_context!(f.node().try_dir())?;
            site_context!(n.add(name.to_string(), || -> AnyResult<_> {
                Ok(Arc::new(Node::from((
                    Link::new(&this.memfs_controller, &Utf8PathBuf::from(link.to_string()))?,
                    Arc::downgrade(f.node()),
                ))))
            }))
            .map(|v| v.is_some())
        })
        .unwrap_or_default()
    }

    #[func]
    fn file_delete_file(&self, path: GString, name: GString, follow_symlink: Variant) -> bool {
        self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let mut n = site_context!(f.node().try_dir())?;

            Ok(n.remove(&name.to_string()))
        })
        .unwrap_or_default()
    }

    #[func]
    fn file_dir_list(&self, path: GString, follow_symlink: Variant) -> Variant {
        option_to_variant(self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let n = site_context!(f.node().try_dir())?;

            Ok(n.iter()
                .map(|(k, _)| GString::from(k))
                .collect::<PackedStringArray>())
        }))
    }

    #[func]
    fn file_stat(&self, path: GString, follow_symlink: Variant) -> Variant {
        option_to_variant(self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let n = &**f.node();

            let mut ret = Dictionary::new();
            ret.set(
                "filetype",
                if n.is_link() {
                    FILE_LINK
                } else if n.is_dir() {
                    FILE_DIR
                } else {
                    FILE_FILE
                },
            );
            let (len, stamp) = n.len_and_stamp();
            ret.set("size", len as u64);
            ret.set("atime", to_unix_time(stamp.atime) as i64);
            ret.set("mtime", to_unix_time(stamp.mtime) as i64);
            ret.set("ctime", to_unix_time(stamp.ctime) as i64);
            Ok(ret)
        }))
    }

    #[func]
    fn file_set_time(&self, path: GString, time: Dictionary, follow_symlink: Variant) -> bool {
        self.wrap_data(move |this| {
            let mtime = time
                .get("mtime")
                .map(variant_to_option::<i64>)
                .transpose()?
                .flatten();
            let atime = time
                .get("atime")
                .map(variant_to_option::<i64>)
                .transpose()?
                .flatten();

            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let mut stamp = f.node().stamp();

            if let Some(t) = mtime.and_then(from_unix_time) {
                stamp.mtime = t;
            }
            if let Some(t) = atime.and_then(from_unix_time) {
                stamp.atime = t;
            }

            Ok(())
        })
        .is_some()
    }

    #[func]
    fn file_link_target(&self, path: GString, follow_symlink: Variant) -> Variant {
        option_to_variant(self.wrap_data(move |this| {
            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            site_context!(f.read_link())
        }))
    }

    #[func]
    fn file_read(
        &self,
        path: GString,
        length: u64,
        offset: Variant,
        follow_symlink: Variant,
    ) -> Variant {
        option_to_variant(self.wrap_data(move |this| {
            let mut off = variant_to_option::<u64>(offset)?.unwrap_or(0) as usize;

            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    None,
                    AccessMode::RW,
                )
            )?;
            let mut n = site_context!(f.node().try_file())?;

            let mut ret = vec![0u8; length as usize];
            let mut s = &mut ret[..];
            while !s.is_empty() {
                let (v, n) = n.read(s.len(), off);
                s[..v.len()].copy_from_slice(v);
                s = &mut s[n..];
                off += n;
            }

            Ok(PackedByteArray::from(ret))
        }))
    }

    #[func]
    fn file_write(
        &self,
        path: GString,
        data: Variant,
        offset: Variant,
        truncate: Variant,
        follow_symlink: Variant,
    ) -> bool {
        fn write_it<T, const N: usize>(
            f: &mut File,
            mut off: usize,
            it: impl IntoIterator<Item = T>,
            c: impl Fn(T, &mut [u8; N]),
        ) -> AnyResult<()> {
            let mut buf = [[0u8; N]; 4];
            let mut i = 0;
            for v in it {
                c(v, &mut buf[i]);

                i += 1;
                if i == buf.len() {
                    i = 0;
                    f.write(buf.as_flattened(), off)?;
                    off += N * buf.len();
                }
            }

            if i > 0 {
                f.write(buf[..i].as_flattened(), off)?;
            }
            Ok(())
        }

        self.wrap_data(move |this| {
            let off = variant_to_option::<u64>(offset)?.unwrap_or(0) as usize;

            let f = site_context!(
                CapWrapper::new(this.memfs_controller.root(), AccessMode::RW).open(
                    &this.memfs_controller,
                    &Utf8PathBuf::from(path.to_string()),
                    site_context!(variant_to_option(follow_symlink))?.unwrap_or(false),
                    Some(CreateParams::new()),
                    AccessMode::RW,
                )
            )?;
            let mut n = site_context!(f.node().try_file())?;

            if variant_to_option::<bool>(truncate)?.unwrap_or(false) {
                site_context!(n.resize(0))?;
            }

            variant_dispatch!{data {
                PACKED_BYTE_ARRAY => site_context!(n.write(data.as_slice(), off))?,
                STRING => site_context!(n.write(data.to_string().as_bytes(), off))?,
                PACKED_INT32_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), |v, s| *s = v.to_le_bytes()))?,
                PACKED_INT64_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), |v, s| *s = v.to_le_bytes()))?,
                PACKED_FLOAT32_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), |v, s| *s = v.to_le_bytes()))?,
                PACKED_FLOAT64_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), |v, s| *s = v.to_le_bytes()))?,
                PACKED_VECTOR2_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), StructPacking::<f32>::write_array))?,
                PACKED_VECTOR3_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), StructPacking::<f32>::write_array))?,
                PACKED_COLOR_ARRAY => site_context!(write_it(&mut n, off, data.as_slice(), StructPacking::<f32>::write_array))?,
                _ => bail_with_site!("Unknown value type {:?}", data.get_type()),
            }};

            Ok(())
        }).is_some()
    }
}
