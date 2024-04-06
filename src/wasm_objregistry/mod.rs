mod funcs;

use std::mem;

use godot::prelude::*;
use slab::Slab;

pub use funcs::Funcs;

use crate::godot_util::SendSyncWrapper;

pub struct ObjectRegistry {
    slab: Slab<SendSyncWrapper<Variant>>,
}

impl Default for ObjectRegistry {
    #[inline]
    fn default() -> Self {
        Self { slab: Slab::new() }
    }
}

impl ObjectRegistry {
    #[inline]
    pub fn get(&self, ix: usize) -> Option<Variant> {
        match ix.checked_sub(1) {
            Some(ix) => self.slab.get(ix).map(|v| &**v).cloned(),
            None => None,
        }
    }

    #[inline]
    pub fn register(&mut self, v: Variant) -> usize {
        if v.is_nil() {
            0
        } else {
            self.slab.insert(SendSyncWrapper::new(v)) + 1
        }
    }

    #[inline]
    pub fn unregister(&mut self, ix: usize) -> Option<Variant> {
        match ix.checked_sub(1) {
            Some(ix) => self.slab.try_remove(ix).map(|v| v.into_inner()),
            None => None,
        }
    }

    #[inline]
    pub fn replace(&mut self, ix: usize, v: Variant) -> Option<Variant> {
        if v.is_nil() {
            return self.unregister(ix);
        }
        ix.checked_sub(1)
            .and_then(|ix| self.slab.get_mut(ix))
            .map(|p| mem::replace(p, SendSyncWrapper::new(v)).into_inner())
    }

    #[inline]
    pub fn get_or_nil(&self, ix: usize) -> Variant {
        self.get(ix).unwrap_or_default()
    }
}
