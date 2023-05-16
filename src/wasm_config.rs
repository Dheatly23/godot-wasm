use std::collections::HashMap;

use gdnative::prelude::*;

#[cfg(feature = "wasi")]
use crate::wasi_ctx::WasiContext;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::{EPOCH_DEADLINE, EPOCH_MULTIPLIER};

#[derive(Clone, Default, Debug, ToVariant)]
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
    pub wasi_context: Option<Instance<WasiContext>>,
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

    pub extern_bind: ExternBindingType,
}

fn get_field<T: FromVariant>(
    d: &Dictionary,
    name: &'static str,
) -> Result<Option<T>, FromVariantError> {
    match d.get(name) {
        Some(v) => match T::from_variant(&v) {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(FromVariantError::InvalidField {
                field_name: name,
                error: Box::new(e),
            }),
        },
        None => Ok(None),
    }
}

#[cfg(feature = "epoch-timeout")]
fn compute_epoch(v: Option<Variant>) -> Result<u64, FromVariantError> {
    const DEFAULT: u64 = EPOCH_DEADLINE.saturating_mul(EPOCH_MULTIPLIER);
    let t = v.as_ref().map_or(VariantType::Nil, |v| v.get_type());
    match v.map(|v| v.dispatch()) {
        None | Some(VariantDispatch::Nil) => Ok(DEFAULT),
        Some(VariantDispatch::I64(v)) => Ok(v
            .try_into()
            .unwrap_or(0u64)
            .saturating_mul(EPOCH_MULTIPLIER)),
        Some(VariantDispatch::F64(v)) => Ok((v * (EPOCH_MULTIPLIER as f64)).trunc() as _),
        Some(_) => Err(FromVariantError::InvalidField {
            field_name: "engine.epoch_timeout",
            error: Box::new(FromVariantError::InvalidVariantType {
                variant_type: t,
                expected: VariantType::F64,
            }),
        }),
    }
    .map(|i| i.max(1))
}

impl FromVariant for Config {
    fn from_variant(variant: &Variant) -> Result<Self, FromVariantError> {
        if variant.is_nil() {
            return Ok(Self::default());
        }
        let dict = Dictionary::from_variant(variant)?;

        Ok(Self {
            #[cfg(feature = "epoch-timeout")]
            with_epoch: get_field(&dict, "engine.use_epoch")?.unwrap_or_default(),
            #[cfg(feature = "epoch-timeout")]
            epoch_autoreset: get_field(&dict, "engine.epoch_autoreset")?.unwrap_or_default(),
            #[cfg(feature = "epoch-timeout")]
            epoch_timeout: compute_epoch(dict.get("engine.epoch_timeout"))?,

            #[cfg(feature = "memory-limiter")]
            max_memory: get_field(&dict, "engine.max_memory")?,
            #[cfg(feature = "memory-limiter")]
            max_entries: get_field(&dict, "engine.max_entries")?,

            #[cfg(feature = "wasi")]
            with_wasi: get_field(&dict, "engine.use_wasi")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_context: get_field(&dict, "wasi.wasi_context")?,
            #[cfg(feature = "wasi")]
            wasi_args: get_field(&dict, "wasi.args")?.unwrap_or_default(),
            #[cfg(feature = "wasi")]
            wasi_envs: get_field(&dict, "wasi.envs")?.unwrap_or_default(),
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

            extern_bind: get_field(&dict, "godot.extern_binding")?.unwrap_or_default(),
        })
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

impl FromVariant for ExternBindingType {
    fn from_variant(variant: &Variant) -> Result<Self, FromVariantError> {
        let s = String::from_variant(variant)?;
        Ok(match &*s {
            "" | "none" | "no_binding" => Self::None,
            #[cfg(feature = "object-registry-compat")]
            "compat" | "registry" => Self::Registry,
            #[cfg(feature = "object-registry-extern")]
            "extern" | "native" => Self::Native,
            _ => {
                return Err(FromVariantError::UnknownEnumVariant {
                    variant: s,
                    expected: &[],
                })
            }
        })
    }
}

impl ToVariant for ExternBindingType {
    fn to_variant(&self) -> Variant {
        match self {
            Self::None => "none",
            #[cfg(feature = "object-registry-compat")]
            Self::Registry => "registry",
            #[cfg(feature = "object-registry-extern")]
            Self::Native => "native",
        }
        .to_variant()
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

impl FromVariant for PipeBindingType {
    fn from_variant(variant: &Variant) -> Result<Self, FromVariantError> {
        if variant.is_nil() {
            return Ok(Self::default());
        }

        let s = String::from_variant(variant)?;
        Ok(match &*s {
            "" | "unbound" => Self::Unbound,
            "instance" => Self::Instance,
            "context" => Self::Context,
            _ => {
                return Err(FromVariantError::UnknownEnumVariant {
                    variant: s,
                    expected: &[],
                })
            }
        })
    }
}

impl ToVariant for PipeBindingType {
    fn to_variant(&self) -> Variant {
        match self {
            Self::Unbound => "unbound",
            Self::Instance => "instance",
            Self::Context => "context",
        }
        .to_variant()
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

impl FromVariant for PipeBufferType {
    fn from_variant(variant: &Variant) -> Result<Self, FromVariantError> {
        if variant.is_nil() {
            return Ok(Self::default());
        }

        let s = String::from_variant(variant)?;
        Ok(match &*s {
            "" | "unbuffered" => Self::Unbuffered,
            "line" => Self::LineBuffer,
            "block" => Self::BlockBuffer,
            _ => {
                return Err(FromVariantError::UnknownEnumVariant {
                    variant: s,
                    expected: &[],
                })
            }
        })
    }
}

impl ToVariant for PipeBufferType {
    fn to_variant(&self) -> Variant {
        match self {
            Self::Unbuffered => "unbuffered",
            Self::LineBuffer => "line",
            Self::BlockBuffer => "block",
        }
        .to_variant()
    }
}
