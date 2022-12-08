use gdnative::prelude::*;

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq, ToVariant)]
pub struct Config {
    #[cfg(feature = "epoch-timeout")]
    pub with_epoch: bool,
    #[cfg(feature = "epoch-timeout")]
    pub epoch_autoreset: bool,

    pub extern_bind: ExternBindingType,
}

fn get_field<T: FromVariant + Default>(
    d: &Dictionary,
    name: &'static str,
) -> Result<T, FromVariantError> {
    match d.get(name) {
        Some(v) => T::from_variant(&v).map_err(|e| FromVariantError::InvalidField {
            field_name: name,
            error: Box::new(e),
        }),
        None => Ok(T::default()),
    }
}

impl FromVariant for Config {
    fn from_variant(variant: &Variant) -> Result<Self, FromVariantError> {
        if variant.is_nil() {
            return Ok(Self::default());
        }
        let dict = Dictionary::from_variant(variant)?;

        Ok(Self {
            #[cfg(feature = "epoch-timeout")]
            with_epoch: get_field(&dict, "engine.use_epoch")?,
            #[cfg(feature = "epoch-timeout")]
            epoch_autoreset: get_field(&dict, "engine.epoch_autoreset")?,

            extern_bind: get_field(&dict, "godot.extern_binding")?,
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
