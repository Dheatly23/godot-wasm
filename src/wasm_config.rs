use godot::prelude::*;

#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::{EPOCH_DEADLINE, EPOCH_MULTIPLIER};

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
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

    pub extern_bind: ExternBindingType,
}

fn get_field<T: FromVariant + Default>(
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

impl FromVariant for Config {
    fn try_from_variant(v: &Variant) -> Result<Self, VariantConversionError> {
        if v.is_nil() {
            return Ok(Self::default());
        }
        let dict = Dictionary::try_from_variant(&v)?;

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
    fn try_from_variant(variant: &Variant) -> Result<Self, VariantConversionError> {
        let s = String::try_from_variant(variant)?;
        Ok(match &*s {
            "" | "none" | "no_binding" => Self::None,
            #[cfg(feature = "object-registry-compat")]
            "compat" | "registry" => Self::Registry,
            #[cfg(feature = "object-registry-extern")]
            "extern" | "native" => Self::Native,
            _ => return Err(VariantConversionError),
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
