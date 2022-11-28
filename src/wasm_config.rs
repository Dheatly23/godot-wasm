use gdnative::prelude::*;

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq, ToVariant)]
pub struct Config {
    pub with_epoch: bool,
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
            with_epoch: get_field(&dict, "engine.use_epoch")?,
            extern_bind: get_field(&dict, "godot.extern_binding")?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternBindingType {
    None,
    Registry,
    // XXX: Defer this until later
    //Native,
}

impl Default for ExternBindingType {
    fn default() -> Self {
        Self::None
    }
}

impl FromVariant for ExternBindingType {
    fn from_variant(variant: &Variant) -> Result<Self, FromVariantError> {
        static ALL_VARIANTS: &[&str] = &["none", "no_binding", "compat", "registry"];
        let s = String::from_variant(variant)?;
        Ok(match &*s {
            "" | "none" | "no_binding" => Self::None,
            "compat" | "registry" => Self::Registry,
            _ => {
                return Err(FromVariantError::UnknownEnumVariant {
                    variant: s,
                    expected: ALL_VARIANTS,
                })
            }
        })
    }
}

impl ToVariant for ExternBindingType {
    fn to_variant(&self) -> Variant {
        match self {
            Self::None => "none",
            Self::Registry => "registry",
        }
        .to_variant()
    }
}
