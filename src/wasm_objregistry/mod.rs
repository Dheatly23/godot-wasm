mod funcs;

use std::mem;

use godot::prelude::*;
use slab::Slab;

pub use funcs::Funcs;

pub struct ObjectRegistry {
    slab: Slab<Variant>,
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
            Some(ix) => self.slab.get(ix).cloned(),
            None => None,
        }
    }

    #[inline]
    pub fn register(&mut self, v: Variant) -> usize {
        if v.is_nil() {
            0
        } else {
            self.slab.insert(v) + 1
        }
    }

    #[inline]
    pub fn unregister(&mut self, ix: usize) -> Option<Variant> {
        match ix.checked_sub(1) {
            Some(ix) => self.slab.try_remove(ix),
            None => None,
        }
    }

    #[inline]
    pub fn replace(&mut self, ix: usize, v: Variant) -> Option<Variant> {
        if v.is_nil() {
            return self.unregister(ix);
        }
        ix.checked_sub(1)
            .and_then(|ix| self.slab.get_mut(ix).as_mut().map(|p| mem::replace(*p, v)))
    }

    #[inline]
    pub fn get_or_nil(&self, ix: usize) -> Variant {
        self.get(ix).unwrap_or_else(Variant::nil)
    }
}
