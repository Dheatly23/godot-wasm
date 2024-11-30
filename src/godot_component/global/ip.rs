use anyhow::Result as AnyResult;
use godot::classes::Ip;
use godot::classes::ip::{ResolverStatus, Type};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::bindgen::godot::global::ip;
use crate::{bail_with_site, filter_macro};

fn from_type(v: ip::Type) -> Type {
    match v {
        ip::Type::None => Type::NONE,
        ip::Type::Ipv4 => Type::IPV4,
        ip::Type::Ipv6 => Type::IPV6,
        ip::Type::Any => Type::ANY,
    }
}

filter_macro! {method [
    singleton -> "singleton",
    clear_cache -> "clear-cache",
    erase_resolve_item -> "erase-resolve-item",
    get_local_addresses -> "get-local-addresses",
    get_local_interfaces -> "get-local-interfaces",
    get_resolve_item_address -> "get-resolve-item-address",
    get_resolve_item_addresses -> "get-resolve-item-addresses",
    get_resolve_item_status -> "get-resolve-item-status",
    resolve_hostname -> "resolve-hostname",
    resolve_hostname_addresses -> "resolve-hostname-addresses",
    resolve_hostname_queue_item -> "resolve-hostname-queue-item",
]}

impl ip::Host for crate::godot_component::GodotCtx {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, singleton)?;
        self.set_into_var(Ip::singleton())
    }

    fn clear_cache(&mut self, h: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, clear_cache)?;
        Ip::singleton()
            .clear_cache_ex()
            .hostname(&self.get_value::<GString>(h)?)
            .done();
        Ok(())
    }

    fn erase_resolve_item(&mut self, i: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, erase_resolve_item)?;
        Ip::singleton().erase_resolve_item(i);
        Ok(())
    }

    fn get_local_addresses(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, get_local_addresses)?;
        self.set_into_var(Ip::singleton().get_local_addresses())
    }

    fn get_local_interfaces(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, get_local_interfaces)?;
        self.set_into_var(Ip::singleton().get_local_interfaces())
    }

    fn get_resolve_item_address(&mut self, i: i32) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, get_resolve_item_address)?;
        self.set_into_var(Ip::singleton().get_resolve_item_address(i))
    }

    fn get_resolve_item_addresses(&mut self, i: i32) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, get_resolve_item_addresses)?;
        self.set_into_var(Ip::singleton().get_resolve_item_addresses(i))
    }

    fn get_resolve_item_status(&mut self, i: i32) -> AnyResult<ip::ResolverStatus> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, get_resolve_item_status)?;
        Ok(match Ip::singleton().get_resolve_item_status(i) {
            ResolverStatus::NONE => ip::ResolverStatus::None,
            ResolverStatus::WAITING => ip::ResolverStatus::Waiting,
            ResolverStatus::DONE => ip::ResolverStatus::Done,
            ResolverStatus::ERROR => ip::ResolverStatus::Error,
            v => bail_with_site!("Unknown resolver status {v:?}"),
        })
    }

    fn resolve_hostname(
        &mut self,
        h: WasmResource<Variant>,
        i: ip::Type,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, resolve_hostname)?;
        let r = Ip::singleton()
            .resolve_hostname_ex(&self.get_value::<GString>(h)?)
            .ip_type(from_type(i))
            .done();
        self.set_into_var(r)
    }

    fn resolve_hostname_addresses(
        &mut self,
        h: WasmResource<Variant>,
        i: ip::Type,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, resolve_hostname_addresses)?;
        let r = Ip::singleton()
            .resolve_hostname_addresses_ex(&self.get_value::<GString>(h)?)
            .ip_type(from_type(i))
            .done();
        self.set_into_var(r)
    }

    fn resolve_hostname_queue_item(
        &mut self,
        h: WasmResource<Variant>,
        i: ip::Type,
    ) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, ip, resolve_hostname_queue_item)?;
        Ok(Ip::singleton()
            .resolve_hostname_queue_item_ex(&self.get_value::<GString>(h)?)
            .ip_type(from_type(i))
            .done())
    }
}
