mod classes;
mod core;
pub mod filter;
mod global;

use std::borrow::Cow;

use anyhow::{Result as AnyResult, bail};
use godot::global::Error;
use godot::prelude::*;
use slab::Slab;
use wasmtime::component::{HasSelf, Linker, Resource as WasmResource};

use crate::godot_util::{ErrorWrapper, SendSyncWrapper, from_var_any};
use crate::wasm_instance::InnerLock;
use crate::{bail_with_site, filter_macro};

filter_macro! {module [
    godot_core <core> -> "godot:core",
    godot_reflection <reflection_filter> -> "godot:reflection",
    godot_global <global> -> "godot:global",
]}

mod reflection_filter {
    crate::filter_macro! {interface [
        this <this_filter> -> "this",
    ]}

    mod this_filter {
        crate::filter_macro! {method [
            get_this -> "get-this",
        ]}
    }
}

#[derive(Default)]
pub struct GodotCtx {
    inner_lock: InnerLock,

    table: Slab<SendSyncWrapper<Variant>>,

    pub inst_id: Option<InstanceId>,

    pub filter: filter::Filter,
}

impl AsMut<GodotCtx> for GodotCtx {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl AsRef<InnerLock> for GodotCtx {
    fn as_ref(&self) -> &InnerLock {
        &self.inner_lock
    }
}

impl AsMut<InnerLock> for GodotCtx {
    fn as_mut(&mut self) -> &mut InnerLock {
        &mut self.inner_lock
    }
}

impl GodotCtx {
    pub fn new(inst_id: InstanceId) -> Self {
        Self {
            inst_id: Some(inst_id),
            ..Self::default()
        }
    }

    #[inline]
    pub(crate) fn release_store<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.inner_lock.release_store(f)
    }

    pub fn get_var_borrow(&'_ mut self, res: WasmResource<Variant>) -> AnyResult<Cow<'_, Variant>> {
        let i = res.rep() as usize;
        if res.owned() {
            if let Some(v) = self.table.try_remove(i) {
                return Ok(Cow::Owned(v.into_inner()));
            }
        } else if let Some(v) = self.table.get(i) {
            return Ok(Cow::Borrowed(&**v));
        }

        bail!("index is not valid")
    }

    pub fn get_var(&mut self, res: WasmResource<Variant>) -> AnyResult<Variant> {
        self.get_var_borrow(res).map(|v| v.into_owned())
    }

    pub fn maybe_get_var_borrow(
        &'_ mut self,
        res: Option<WasmResource<Variant>>,
    ) -> AnyResult<Cow<'_, Variant>> {
        match res {
            None => Ok(Cow::Owned(Variant::nil())),
            Some(res) => self.get_var_borrow(res),
        }
    }

    pub fn maybe_get_var(&mut self, res: Option<WasmResource<Variant>>) -> AnyResult<Variant> {
        match res {
            None => Ok(Variant::nil()),
            Some(res) => self.get_var(res),
        }
    }

    pub fn get_value<T: FromGodot>(&mut self, res: WasmResource<Variant>) -> AnyResult<T> {
        self.get_var_borrow(res).and_then(from_var_any)
    }

    pub fn get_object<T: GodotClass>(&mut self, res: WasmResource<Variant>) -> AnyResult<Gd<T>> {
        self.get_value(res)
    }

    pub fn try_insert(&mut self, var: Variant) -> AnyResult<u32> {
        let entry = self.table.vacant_entry();
        let ret = u32::try_from(entry.key())?;
        entry.insert(SendSyncWrapper::new(var));
        Ok(ret)
    }

    pub fn set_var(&mut self, var: Variant) -> AnyResult<Option<WasmResource<Variant>>> {
        if var.is_nil() {
            Ok(None)
        } else {
            self.try_insert(var).map(|v| Some(WasmResource::new_own(v)))
        }
    }

    pub fn set_into_var<V: ToGodot>(&mut self, var: V) -> AnyResult<WasmResource<Variant>> {
        let v = var.to_variant();
        drop(var);
        self.try_insert(v).map(WasmResource::new_own)
    }
}

#[allow(dead_code)]
pub type GVar = Variant;

pub mod bindgen {
    pub use super::GVar;

    wasmtime::component::bindgen!({
        path: "wit",
        ownership: Borrowing {
            duplicate_if_necessary: false
        },
        imports: {
            default: tracing | trappable,
        },
        with: {
            "godot:core/core.godot-var": GVar,
        },
    });
}

type ErrorRes<T = ()> = AnyResult<Result<T, bindgen::godot::core::core::Error>>;

fn wrap_error(e: Error) -> ErrorRes {
    use bindgen::godot::core::core::Error as RetError;
    match e {
        Error::OK => Ok(Ok(())),
        Error::FAILED => Ok(Err(RetError::Failed)),
        Error::ERR_UNAVAILABLE => Ok(Err(RetError::ErrUnavailable)),
        Error::ERR_UNCONFIGURED => Ok(Err(RetError::ErrUnconfigured)),
        Error::ERR_UNAUTHORIZED => Ok(Err(RetError::ErrUnauthorized)),
        Error::ERR_PARAMETER_RANGE_ERROR => Ok(Err(RetError::ErrParameterRangeError)),
        Error::ERR_OUT_OF_MEMORY => Ok(Err(RetError::ErrOutOfMemory)),
        Error::ERR_FILE_NOT_FOUND => Ok(Err(RetError::ErrFileNotFound)),
        Error::ERR_FILE_BAD_DRIVE => Ok(Err(RetError::ErrFileBadDrive)),
        Error::ERR_FILE_BAD_PATH => Ok(Err(RetError::ErrFileBadPath)),
        Error::ERR_FILE_NO_PERMISSION => Ok(Err(RetError::ErrFileNoPermission)),
        Error::ERR_FILE_ALREADY_IN_USE => Ok(Err(RetError::ErrFileAlreadyInUse)),
        Error::ERR_FILE_CANT_OPEN => Ok(Err(RetError::ErrFileCantOpen)),
        Error::ERR_FILE_CANT_WRITE => Ok(Err(RetError::ErrFileCantWrite)),
        Error::ERR_FILE_CANT_READ => Ok(Err(RetError::ErrFileCantRead)),
        Error::ERR_FILE_UNRECOGNIZED => Ok(Err(RetError::ErrFileUnrecognized)),
        Error::ERR_FILE_CORRUPT => Ok(Err(RetError::ErrFileCorrupt)),
        Error::ERR_FILE_MISSING_DEPENDENCIES => Ok(Err(RetError::ErrFileMissingDependencies)),
        Error::ERR_FILE_EOF => Ok(Err(RetError::ErrFileEof)),
        Error::ERR_CANT_OPEN => Ok(Err(RetError::ErrCantOpen)),
        Error::ERR_CANT_CREATE => Ok(Err(RetError::ErrCantCreate)),
        Error::ERR_QUERY_FAILED => Ok(Err(RetError::ErrQueryFailed)),
        Error::ERR_ALREADY_IN_USE => Ok(Err(RetError::ErrAlreadyInUse)),
        Error::ERR_LOCKED => Ok(Err(RetError::ErrLocked)),
        Error::ERR_TIMEOUT => Ok(Err(RetError::ErrTimeout)),
        Error::ERR_CANT_CONNECT => Ok(Err(RetError::ErrCantConnect)),
        Error::ERR_CANT_RESOLVE => Ok(Err(RetError::ErrCantResolve)),
        Error::ERR_CONNECTION_ERROR => Ok(Err(RetError::ErrConnectionError)),
        Error::ERR_CANT_ACQUIRE_RESOURCE => Ok(Err(RetError::ErrCantAcquireResource)),
        Error::ERR_CANT_FORK => Ok(Err(RetError::ErrCantFork)),
        Error::ERR_INVALID_DATA => Ok(Err(RetError::ErrInvalidData)),
        Error::ERR_INVALID_PARAMETER => Ok(Err(RetError::ErrInvalidParameter)),
        Error::ERR_ALREADY_EXISTS => Ok(Err(RetError::ErrAlreadyExists)),
        Error::ERR_DOES_NOT_EXIST => Ok(Err(RetError::ErrDoesNotExist)),
        Error::ERR_DATABASE_CANT_READ => Ok(Err(RetError::ErrDatabaseCantRead)),
        Error::ERR_DATABASE_CANT_WRITE => Ok(Err(RetError::ErrDatabaseCantWrite)),
        Error::ERR_COMPILATION_FAILED => Ok(Err(RetError::ErrCompilationFailed)),
        Error::ERR_METHOD_NOT_FOUND => Ok(Err(RetError::ErrMethodNotFound)),
        Error::ERR_LINK_FAILED => Ok(Err(RetError::ErrLinkFailed)),
        Error::ERR_SCRIPT_FAILED => Ok(Err(RetError::ErrScriptFailed)),
        Error::ERR_CYCLIC_LINK => Ok(Err(RetError::ErrCyclicLink)),
        Error::ERR_INVALID_DECLARATION => Ok(Err(RetError::ErrInvalidDeclaration)),
        Error::ERR_DUPLICATE_SYMBOL => Ok(Err(RetError::ErrDuplicateSymbol)),
        Error::ERR_PARSE_ERROR => Ok(Err(RetError::ErrParseError)),
        Error::ERR_BUSY => Ok(Err(RetError::ErrBusy)),
        Error::ERR_SKIP => Ok(Err(RetError::ErrSkip)),
        Error::ERR_HELP => Ok(Err(RetError::ErrHelp)),
        Error::ERR_BUG => Ok(Err(RetError::ErrBug)),
        Error::ERR_PRINTER_ON_FIRE => Ok(Err(RetError::ErrPrinterOnFire)),
        e => Err(ErrorWrapper::from(e).into()),
    }
}

impl bindgen::godot::core::core::HostGodotVar for GodotCtx {
    fn drop(&mut self, rep: WasmResource<Variant>) -> AnyResult<()> {
        self.get_var(rep)?;
        Ok(())
    }

    fn clone(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let v = self.get_var(var)?;
        Ok(WasmResource::new_own(self.try_insert(v)?))
    }
}

impl bindgen::godot::core::core::Host for GodotCtx {
    fn var_equals(
        &mut self,
        a: WasmResource<Variant>,
        b: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, core, var_equals)?;
        Ok(self.get_var(a)? == self.get_var(b)?)
    }

    fn var_hash(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_core, core, var_hash)?;
        Ok(self.get_var(var)?.hash_u32().into())
    }

    fn var_stringify(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        filter_macro!(filter self.filter.as_ref(), godot_core, core, var_stringify)?;
        Ok(self.get_var(var)?.to_string())
    }
}

impl bindgen::godot::reflection::this::Host for GodotCtx {
    fn get_this(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_reflection, this, get_this)?;
        let Some(id) = self.inst_id else {
            bail_with_site!("Self instance ID is not set")
        };

        self.set_into_var(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())?)
    }
}

pub trait HasGodotCtx {
    fn get_ctx(&mut self) -> &mut GodotCtx;
}

impl<T: AsMut<GodotCtx>> HasGodotCtx for T {
    fn get_ctx(&mut self) -> &mut GodotCtx {
        self.as_mut()
    }
}

pub fn add_to_linker<T: 'static + HasGodotCtx>(linker: &mut Linker<T>) -> AnyResult<()> {
    let f: fn(&mut T) -> &mut GodotCtx = <T as HasGodotCtx>::get_ctx;

    bindgen::godot::core::core::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::typeis::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::primitive::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::byte_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::int32_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::int64_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::float32_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::float64_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::vector2_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::vector3_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::color_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::string_array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::array::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::dictionary::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::object::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::callable::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::core::signal::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;

    bindgen::godot::global::globalscope::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::global::classdb::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::global::engine::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::global::input::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::global::input_map::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;
    bindgen::godot::global::ip::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)?;

    bindgen::godot::reflection::this::add_to_linker::<T, HasSelf<GodotCtx>>(&mut *linker, f)
}
