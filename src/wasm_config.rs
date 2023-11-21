use std::collections::HashMap;

use godot::prelude::*;
use godot::builtin::meta::GodotConvert;

#[cfg(feature = "wasi")]
use crate::wasi_ctx::WasiContext;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::{EPOCH_DEADLINE, EPOCH_MULTIPLIER};

#[derive(Default, Debug)]
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
    #[cfg(feature = "wasi")]
    pub wasi_stdin_file: Option<String>,

    pub extern_bind: ExternBindingType,
}

fn get_field<T: FromGodot>(
    d: &Dictionary,
    name: &'static str,
) -> Result<Option<T>, VariantConversionError> {
    match d.get(name) {
        Some(v) => Some(T::try_from_variant(&v)).transpose(),
        None => Ok(None),
    }
}

#[cfg(feature = "epoch-timeout")]
fn compute_epoch(v: Option<Variant>) -> Result<u64, VariantConversionError> {
    const DEFAULT: u64 = EPOCH_DEADLINE.saturating_mul(EPOCH_MULTIPLIER);
    let v = match v {
        Some(v) if !v.is_nil() => v,
        _ => return Ok(DEFAULT),
    };
    if let Ok(v) = i64::try_from_variant(&v) {
        Ok(v.try_into()
            .unwrap_or(0u64)
            .saturating_mul(EPOCH_MULTIPLIER))
    } else {
        Ok((f64::try_from_variant(&v)? * (EPOCH_MULTIPLIER as f64)).trunc() as _)
    }
}

#[cfg(feature = "wasi")]
fn get_wasi_args(v: Option<Variant>) -> Result<Vec<String>, VariantConversionError> {
    let v = match v {
        Some(v) => <Array<Variant>>::try_from_variant(&v)?,
        None => return Ok(Vec::new()),
    };
    let mut ret = Vec::with_capacity(v.len());
    for i in v.iter_shared() {
        ret.push(String::try_from_variant(&i)?);
    }
    Ok(ret)
}

#[cfg(feature = "wasi")]
fn get_wasi_envs(v: Option<Variant>) -> Result<HashMap<String, String>, VariantConversionError> {
    let v = match v {
        Some(v) => Dictionary::try_from_variant(&v)?,
        None => return Ok(HashMap::new()),
    };
    let mut ret = HashMap::with_capacity(v.len());
    for (k, v) in v.iter_shared() {
        ret.insert(String::try_from_variant(&k)?, String::try_from_variant(&v)?);
    }
    Ok(ret)
}

impl Config {
    fn convert(dict: Dictionary) -> Result<Self, VariantConversionError> {
        Ok(Self {
            #[cfg(feature = "epoch-timeout")]
            with_epoch: get_field(&dict, "engine.use_epoch")?.unwrap_or_default(),
            #[cfg(feature = "epoch-timeout")]
            epoch_autoreset: get_field(&dict, "engine.epoch_autoreset")?.unwrap_or_default(),
            #[cfg(feature = "epoch-timeout")]
            epoch_timeout: compute_epoch(dict.get("engine.epoch_timeout"))?,

            #[cfg(feature = "memory-limiter")]
            max_memory: get_field::<i64>(&dict, "engine.max_memory")?.map(|v| v as _),
            #[cfg(feature = "memory-limiter")]
            max_entries: get_field::<i64>(&dict, "engine.max_entries")?.map(|v| v as _),

            #[cfg(feature = "wasi")]
            with_wasi: get_field(&dict, "engine.use_wasi")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_context: get_field(&dict, "wasi.wasi_context")?,
            #[cfg(feature = "wasi")]
            wasi_args: get_wasi_args(dict.get("wasi.args"))?,
            #[cfg(feature = "wasi")]
            wasi_envs: get_wasi_envs(dict.get("wasi.envs"))?,
            #[cfg(feature = "wasi")]
            wasi_fs_readonly: get_field(&dict, "wasi.fs_readonly")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdin: get_field(&dict, "wasi.stdin")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdout: get_field(&dict, "wasi.stdout")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stderr: get_field(&dict, "wasi.stderr")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdout_buffer: get_field(&dict, "wasi.stdout_buffer")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stderr_buffer: get_field(&dict, "wasi.stderr_buffer")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_stdin_data: get_field(&dict, "wasi.stdin_data")?,
            #[cfg(feature = "wasi")]
            wasi_stdin_file: get_field(&dict, "wasi.stdin_file")?,

            extern_bind: get_field(&dict, "godot.extern_binding")?.unwrap_or_default(),
        })
    }
}

impl GodotConvert for Config {
    type Via = Dictionary;
}

impl FromGodot for Config {
    fn try_from_variant(v: &Variant) -> Result<Self, VariantConversionError> {
        if v.is_nil() {
            return Ok(Self::default());
        }
        Self::convert(Dictionary::try_from_variant(v)?)
    }

    fn try_from_godot(via: Self::Via) -> Option<Self> {
        Self::convert(via).ok()
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
    fn try_from_variant(variant: &Variant) -> Result<Self, VariantConversionError> {
        match Self::try_from_godot(GString::try_from_variant(variant)?) {
            Some(v) => Ok(v),
            None => Err(VariantConversionError::BadValue),
        }
    }

    fn try_from_godot(via: Self::Via) -> Option<Self> {
        // SAFETY: Eh whatevers, if it blows up i assume no responsibility
        let chars = unsafe { via.chars_unchecked() };

        match chars {
            [] | ['n', 'o', 'n', 'e'] | ['n', 'o', '_', 'b', 'i', 'n', 'd', 'i', 'n', 'g'] => Some(Self::None),
            #[cfg(feature = "object-registry-compat")]
            ['c', 'o', 'm', 'p', 'a', 't'] | ['r', 'e', 'g', 'i', 's', 't', 'r', 'y'] => Some(Self::Registry),
            #[cfg(feature = "object-registry-extern")]
            ['e', 'x', 't', 'e', 'r', 'n'] | ['n', 'a', 't', 'i', 'v', 'e'] => Some(Self::Native),
            _ => None,
        }
    }
}

impl ToGodot for ExternBindingType {
    fn to_godot(&self) -> Self::Via {
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
    Instance,
    Context,
}

impl Default for PipeBindingType {
    fn default() -> Self {
        Self::Context
    }
}

impl GodotConvert for PipeBindingType {
    type Via = GString;
}

impl FromGodot for PipeBindingType {
    fn try_from_variant(variant: &Variant) -> Result<Self, VariantConversionError> {
        if variant.is_nil() {
            return Ok(Self::default());
        }

        match Self::try_from_godot(GString::try_from_variant(variant)?) {
            Some(v) => Ok(v),
            None => Err(VariantConversionError::BadValue),
        }
    }

    fn try_from_godot(via: Self::Via) -> Option<Self> {
        // SAFETY: Eh whatevers, if it blows up i assume no responsibility
        let chars = unsafe { via.chars_unchecked() };

        match chars {
            [] | ['u', 'n', 'b', 'o', 'u', 'n', 'd'] => Some(Self::Unbound),
            ['i', 'n', 's', 't', 'a', 'n', 'c', 'e'] => Some(Self::Instance),
            ['c', 'o', 'n', 't', 'e', 'x', 't'] => Some(Self::Context),
            _ => None,
        }
    }
}

impl ToGodot for PipeBindingType {
    fn to_godot(&self) -> Self::Via {
        match self {
            Self::Unbound => "unbound",
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

impl Default for PipeBufferType {
    fn default() -> Self {
        Self::LineBuffer
    }
}

impl GodotConvert for PipeBufferType {
    type Via = GString;
}

impl FromGodot for PipeBufferType {
    fn try_from_variant(variant: &Variant) -> Result<Self, VariantConversionError> {
        if variant.is_nil() {
            return Ok(Self::default());
        }

        match Self::try_from_godot(GString::try_from_variant(variant)?) {
            Some(v) => Ok(v),
            None => Err(VariantConversionError::BadValue),
        }
    }

    fn try_from_godot(via: Self::Via) -> Option<Self> {
        // SAFETY: Eh whatevers, if it blows up i assume no responsibility
        let chars = unsafe { via.chars_unchecked() };

        match chars {
            [] | ['u', 'n', 'b', 'u', 'f', 'f', 'e', 'r', 'e', 'd'] => Some(Self::Unbuffered),
            ['l', 'i', 'n', 'e'] => Some(Self::LineBuffer),
            ['b', 'l', 'o', 'c', 'k'] => Some(Self::BlockBuffer),
            _ => None,
        }
    }
}

impl ToGodot for PipeBufferType {
    fn to_godot(&self) -> Self::Via {
        match self {
            Self::Unbuffered => "unbuffered",
            Self::LineBuffer => "line",
            Self::BlockBuffer => "block",
        }
        .into()
    }
}
