mod classes;
mod core;
mod global;

use std::borrow::Cow;

use anyhow::{bail, Result as AnyResult};
use godot::engine::global::Error;
use godot::prelude::*;
use slab::Slab;
use wasmtime::component::{Linker, Resource as WasmResource};

use crate::bail_with_site;
use crate::godot_util::{ErrorWrapper, SendSyncWrapper};

#[derive(Default)]
pub struct GodotCtx {
    table: Slab<SendSyncWrapper<Variant>>,
    pub inst_id: Option<InstanceId>,
    pub allow_unsafe_behavior: bool,
}

impl AsMut<GodotCtx> for GodotCtx {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl GodotCtx {
    pub fn new(inst_id: InstanceId) -> Self {
        Self {
            inst_id: Some(inst_id),
            ..Self::default()
        }
    }

    pub fn get_var_borrow(&mut self, res: WasmResource<Variant>) -> AnyResult<Cow<Variant>> {
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
        &mut self,
        res: Option<WasmResource<Variant>>,
    ) -> AnyResult<Cow<Variant>> {
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
        interfaces: "
            include godot:core/imports@0.1.0;
            include godot:reflection/imports@0.1.0;
            include godot:global/imports@0.1.0;
        ",
        tracing: false,
        async: false,
        ownership: Borrowing {
            duplicate_if_necessary: false
        },
        with: {
            "godot:core/core/godot-var": GVar,
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

impl<T: AsMut<GodotCtx>> bindgen::godot::core::core::HostGodotVar for T {
    fn drop(&mut self, rep: WasmResource<Variant>) -> AnyResult<()> {
        self.as_mut().get_var(rep)?;
        Ok(())
    }

    fn clone(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v = this.get_var(var)?;
        Ok(WasmResource::new_own(this.try_insert(v)?))
    }
}

impl<T: AsMut<GodotCtx>> bindgen::godot::core::core::Host for T {
    fn var_equals(
        &mut self,
        a: WasmResource<Variant>,
        b: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        Ok(this.get_var(a)? == this.get_var(b)?)
    }

    fn var_hash(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(self.as_mut().get_var(var)?.hash())
    }

    fn var_stringify(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self.as_mut().get_var(var)?.to_string())
    }
}

impl<T: AsMut<GodotCtx>> bindgen::godot::reflection::this::Host for T {
    fn get_this(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let Some(id) = this.inst_id else {
            bail_with_site!("Self instance ID is not set")
        };

        this.set_into_var(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())?)
    }
}

pub fn add_to_linker<T, U: AsMut<GodotCtx>>(
    linker: &mut Linker<T>,
    f: impl Fn(&mut T) -> &mut U + Send + Sync + Copy + 'static,
) -> AnyResult<()> {
    bindgen::godot::core::core::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::typeis::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::primitive::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::byte_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::int32_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::int64_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::float32_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::float64_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::vector2_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::vector3_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::color_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::string_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::dictionary::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::object::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::callable::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::signal::add_to_linker(&mut *linker, f)?;

    bindgen::godot::global::globalscope::add_to_linker(&mut *linker, f)?;
    bindgen::godot::global::classdb::add_to_linker(&mut *linker, f)?;

    bindgen::godot::reflection::this::add_to_linker(&mut *linker, f)
}
