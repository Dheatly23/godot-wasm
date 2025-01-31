#[cfg(feature = "wasi")]
use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result as FmtResult};

use godot::prelude::*;
use tracing::warn;

use crate::godot_util::to_lower_inline_smol_str;
#[cfg(feature = "epoch-timeout")]
use crate::variant_dispatch;
#[cfg(feature = "wasi")]
use crate::wasi_ctx::WasiContext;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::{EPOCH_DEADLINE, EPOCH_MULTIPLIER};

#[derive(Default)]
pub struct Config {
    #[cfg(feature = "epoch-timeout")]
    pub with_epoch: bool,
    #[cfg(feature = "epoch-timeout")]
    pub epoch_autoreset: bool,
    #[cfg(feature = "epoch-timeout")]
    pub epoch_timeout: u64,

    #[cfg(feature = "memory-limiter")]
    pub max_memory: Option<u64>,
    #[cfg(feature = "memory-limiter")]
    pub max_entries: Option<u64>,

    #[cfg(feature = "wasi")]
    pub with_wasi: bool,
    #[cfg(feature = "wasi")]
    pub wasi_context: Option<Gd<WasiContext>>,
    #[cfg(feature = "wasi")]
    pub wasi_args: Vec<String>,
    #[cfg(feature = "wasi")]
    pub wasi_envs: HashMap<String, String>,
    #[cfg(feature = "wasi")]
    pub wasi_fs_readonly: bool,
    #[cfg(feature = "wasi")]
    pub wasi_stdin: PipeBindingType,
    #[cfg(feature = "wasi")]
    pub wasi_stdout: PipeBindingType,
    #[cfg(feature = "wasi")]
    pub wasi_stderr: PipeBindingType,
    #[cfg(feature = "wasi")]
    pub wasi_stdout_buffer: PipeBufferType,
    #[cfg(feature = "wasi")]
    pub wasi_stderr_buffer: PipeBufferType,
    #[cfg(feature = "wasi")]
    pub wasi_stdin_data: Option<PackedByteArray>,
    //#[cfg(feature = "wasi")]
    //pub wasi_stdin_file: Option<String>,

    // Not worth cfg() it
    #[allow(dead_code)]
    pub extern_bind: ExternBindingType,
}

impl Debug for Config {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut f = f.debug_struct("Config");
        #[cfg(feature = "epoch-timeout")]
        f.field("with_epoch", &self.with_epoch);
        #[cfg(feature = "epoch-timeout")]
        f.field("epoch_autoreset", &self.epoch_autoreset);
        #[cfg(feature = "epoch-timeout")]
        f.field("epoch_timeout", &self.epoch_timeout);

        #[cfg(feature = "memory-limiter")]
        f.field("max_memory", &self.max_memory);
        #[cfg(feature = "memory-limiter")]
        f.field("max_entries", &self.max_entries);

        #[cfg(feature = "wasi")]
        f.field("with_wasi", &self.with_wasi);
        #[cfg(feature = "wasi")]
        f.field("wasi_context", &self.wasi_context);
        #[cfg(feature = "wasi")]
        f.field("wasi_args", &self.wasi_args);
        #[cfg(feature = "wasi")]
        f.field("wasi_fs_readonly", &self.wasi_fs_readonly);
        #[cfg(feature = "wasi")]
        f.field("wasi_stdin", &self.wasi_stdin);
        #[cfg(feature = "wasi")]
        f.field("wasi_stdout", &self.wasi_stdout);
        #[cfg(feature = "wasi")]
        f.field("wasi_stderr", &self.wasi_stderr);
        #[cfg(feature = "wasi")]
        f.field("wasi_stdout_buffer", &self.wasi_stdout_buffer);
        #[cfg(feature = "wasi")]
        f.field("wasi_stderr_buffer", &self.wasi_stderr_buffer);
        #[cfg(feature = "wasi")]
        f.field(
            "wasi_stdin_data_len",
            &self.wasi_stdin_data.as_ref().map(|v| v.len()),
        );

        f.field("extern_bind", &self.extern_bind);
        f.finish_non_exhaustive()
    }
}

fn get_field<T: FromGodot>(
    d: &Dictionary,
    names: impl IntoIterator<Item = &'static str>,
) -> Result<Option<T>, ConvertError> {
    for name in names {
        if let Some(v) = d.get(name) {
            return Some(v.try_to()).transpose();
        }
    }

    Ok(None)
}

#[cfg(feature = "epoch-timeout")]
fn compute_epoch(v: Option<Variant>) -> Result<u64, ConvertError> {
    const DEFAULT: u64 = EPOCH_DEADLINE.saturating_mul(EPOCH_MULTIPLIER);
    Ok(match v {
        None => DEFAULT,
        Some(v) => variant_dispatch!(v {
            NIL => DEFAULT,
            INT => (v as u64).saturating_mul(EPOCH_MULTIPLIER),
            FLOAT => (v * (EPOCH_MULTIPLIER as f64)).trunc() as _,
            _ => return Err(ConvertError::with_error_value("Unknown value", v)),
        }),
    }
    .max(1))
}

#[cfg(feature = "wasi")]
fn get_wasi_args(v: Option<Variant>) -> Result<Vec<String>, ConvertError> {
    let v = match v {
        Some(v) => v.try_to::<VariantArray>()?,
        None => return Ok(Vec::new()),
    };
    let mut ret = Vec::with_capacity(v.len());
    for i in v.iter_shared() {
        ret.push(i.try_to::<String>()?);
    }
    Ok(ret)
}

#[cfg(feature = "wasi")]
fn get_wasi_envs(v: Option<Variant>) -> Result<HashMap<String, String>, ConvertError> {
    let v = match v {
        Some(v) => v.try_to::<Dictionary>()?,
        None => return Ok(HashMap::new()),
    };
    let mut ret = HashMap::with_capacity(v.len());
    for (k, v) in v.iter_shared() {
        ret.insert(k.try_to::<String>()?, v.try_to::<String>()?);
    }
    Ok(ret)
}

impl Config {
    fn convert(dict: Dictionary) -> Result<Self, ConvertError> {
        Ok(Self {
            #[cfg(feature = "epoch-timeout")]
            with_epoch: get_field(&dict, ["epoch.enable", "engine.use_epoch"])?.unwrap_or_default(),
            #[cfg(feature = "epoch-timeout")]
            epoch_autoreset: get_field(&dict, ["epoch.useAutoreset", "engine.epoch_autoreset"])?
                .unwrap_or_default(),
            #[cfg(feature = "epoch-timeout")]
            epoch_timeout: compute_epoch(
                dict.get("epoch.timeout")
                    .or_else(|| dict.get("engine.epoch_timeout")),
            )?,

            #[cfg(feature = "memory-limiter")]
            max_memory: get_field::<i64>(&dict, ["memory.maxGrowBytes", "engine.max_memory"])?
                .map(|v| v as _),
            #[cfg(feature = "memory-limiter")]
            max_entries: get_field::<i64>(&dict, ["table.maxGrowEntries", "engine.max_entries"])?
                .map(|v| v as _),

            #[cfg(feature = "wasi")]
            with_wasi: get_field(&dict, ["wasi.enable", "engine.use_wasi"])?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_context: get_field(&dict, ["wasi.context", "wasi.wasi_context"])?,
            #[cfg(feature = "wasi")]
            wasi_args: get_wasi_args(dict.get("wasi.args"))?,
            #[cfg(feature = "wasi")]
            wasi_envs: get_wasi_envs(dict.get("wasi.envs"))?,
            #[cfg(feature = "wasi")]
            wasi_fs_readonly: get_field(&dict, ["wasi.fsReadonly", "wasi.fs_readonly"])?
                .unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdin: get_field::<PipeBindingType>(&dict, ["wasi.stdin.bindMode", "wasi.stdin"])?
                .inspect(|&v| {
                    if let PipeBindingType::Bypass | PipeBindingType::Context = v {
                        warn!(binding = ?v, "Stdin binding type is unsupported.");
                        godot_warn!("Stdin binding type {v:?} is unsupported.");
                    }
                })
                .unwrap_or(PipeBindingType::Unbound),
            #[cfg(feature = "wasi")]
            wasi_stdout: get_field(&dict, ["wasi.stdout.bindMode", "wasi.stdout"])?
                .unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stderr: get_field(&dict, ["wasi.stderr.bindMode", "wasi.stderr"])?
                .unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdout_buffer: get_field(&dict, ["wasi.stdout.bufferMode", "wasi.stdout_buffer"])?
                .unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stderr_buffer: get_field(&dict, ["wasi.stderr.bufferMode", "wasi.stderr_buffer"])?
                .unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdin_data: get_field(&dict, ["wasi.stdin.inputData", "wasi.stdin_data"])?,
            //#[cfg(feature = "wasi")]
            //wasi_stdin_file: get_field(&dict, ["wasi.stdin.inputFile", "wasi.stdin_file"])?,
            extern_bind: get_field(&dict, ["extern.bindMode", "godot.extern_binding"])?
                .unwrap_or_default(),
        })
    }
}

impl GodotConvert for Config {
    type Via = Dictionary;
}

impl FromGodot for Config {
    fn try_from_variant(v: &Variant) -> Result<Self, ConvertError> {
        if v.is_nil() {
            return Ok(Self::default());
        }
        Self::convert(v.try_to()?)
    }

    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        Self::convert(via)
    }

    fn from_godot(via: Self::Via) -> Self {
        Self::convert(via).unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExternBindingType {
    None,
    #[cfg(feature = "object-registry-compat")]
    Registry,
    #[cfg(feature = "object-registry-extern")]
    Native,
}

impl Default for ExternBindingType {
    fn default() -> Self {
        Self::None
    }
}

impl GodotConvert for ExternBindingType {
    type Via = GString;
}

impl FromGodot for ExternBindingType {
    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        Ok(match to_lower_inline_smol_str(via.chars()).as_deref() {
            Some("" | "none" | "no_binding") => Self::None,
            #[cfg(feature = "object-registry-compat")]
            Some("compat" | "registry") => Self::Registry,
            #[cfg(feature = "object-registry-extern")]
            Some("extern" | "native") => Self::Native,
            _ => return Err(ConvertError::with_error_value("Unknown value", via)),
        })
    }
}

impl ToGodot for ExternBindingType {
    type ToVia<'a> = Self::Via;

    fn to_godot(&self) -> Self::ToVia<'_> {
        match self {
            Self::None => "none",
            #[cfg(feature = "object-registry-compat")]
            Self::Registry => "registry",
            #[cfg(feature = "object-registry-extern")]
            Self::Native => "native",
        }
        .into()
    }
}

#[cfg(feature = "wasi")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PipeBindingType {
    Unbound,
    Bypass,
    Instance,
    Context,
}

#[cfg(feature = "wasi")]
impl Default for PipeBindingType {
    fn default() -> Self {
        Self::Context
    }
}

#[cfg(feature = "wasi")]
impl GodotConvert for PipeBindingType {
    type Via = GString;
}

#[cfg(feature = "wasi")]
impl FromGodot for PipeBindingType {
    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        Ok(match to_lower_inline_smol_str(via.chars()).as_deref() {
            Some("" | "unbound") => Self::Unbound,
            Some("bypass") => Self::Bypass,
            Some("instance") => Self::Instance,
            Some("context") => Self::Context,
            _ => return Err(ConvertError::with_error_value("Unknown value", via)),
        })
    }
}

#[cfg(feature = "wasi")]
impl ToGodot for PipeBindingType {
    type ToVia<'a> = Self::Via;

    fn to_godot(&self) -> Self::ToVia<'_> {
        match self {
            Self::Unbound => "unbound",
            Self::Bypass => "bypass",
            Self::Instance => "instance",
            Self::Context => "context",
        }
        .into()
    }
}

#[cfg(feature = "wasi")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PipeBufferType {
    Unbuffered,
    LineBuffer,
    BlockBuffer,
}

#[cfg(feature = "wasi")]
impl Default for PipeBufferType {
    fn default() -> Self {
        Self::LineBuffer
    }
}

#[cfg(feature = "wasi")]
impl GodotConvert for PipeBufferType {
    type Via = GString;
}

#[cfg(feature = "wasi")]
impl FromGodot for PipeBufferType {
    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        let chars = via.chars();

        match chars {
            [] | ['u', 'n', 'b', 'u', 'f', 'f', 'e', 'r', 'e', 'd'] => Ok(Self::Unbuffered),
            ['l', 'i', 'n', 'e'] => Ok(Self::LineBuffer),
            ['b', 'l', 'o', 'c', 'k'] => Ok(Self::BlockBuffer),
            _ => Err(ConvertError::with_error_value("Unknown variant", via)),
        }
    }
}

#[cfg(feature = "wasi")]
impl ToGodot for PipeBufferType {
    type ToVia<'a> = Self::Via;

    fn to_godot(&self) -> Self::ToVia<'_> {
        match self {
            Self::Unbuffered => "unbuffered",
            Self::LineBuffer => "line",
            Self::BlockBuffer => "block",
        }
        .into()
    }
}
