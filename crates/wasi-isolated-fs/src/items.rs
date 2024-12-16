use std::io::Read;
use std::mem::replace;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use anyhow::Result as AnyResult;
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::errors;
use crate::fs_isolated::{CapWrapper, DirEntryAccessor, FileAccessor};
use crate::stdio::{
    StderrBypass, StdinSignal, StdinSignalPollable, StdoutBypass, StdoutCbLineBuffered,
};
use crate::NullPollable;

#[derive(Default)]
pub(crate) struct Items {
    data: Vec<MaybeItem>,
    next: usize,
}

enum MaybeItem {
    Item(Item),
    Empty(usize),
}

macro_rules! item_def {
    ($($oi:ident | $oir:ident ($($ot:ty),+ $(,)?) {$($ei:ident ($et:ty)),* $(,)?}),* $(,)?) => {
        #[non_exhaustive]
        pub enum Item {
            $($($ei($et),)*)*
        }

        $($(
        impl From<$et> for Item {
            fn from(v: $et) -> Self {
                Self::$ei(v)
            }
        }

        impl From<Item> for Result<$et, Item> {
            fn from(v: Item) -> Self {
                match v {
                    Item::$ei(v) => Ok(v),
                    v => Err(v),
                }
            }
        }

        impl<'a> From<&'a Item> for Result<&'a $et, &'a Item> {
            fn from(v: &'a Item) -> Self {
                match v {
                    Item::$ei(v) => Ok(v),
                    v => Err(v),
                }
            }
        }

        impl<'a> From<&'a mut Item> for Result<&'a mut $et, &'a mut Item> {
            fn from(v: &'a mut Item) -> Self {
                match v {
                    Item::$ei(v) => Ok(v),
                    v => Err(v),
                }
            }
        }
        )*

        #[allow(clippy::enum_variant_names)]
        pub(crate) enum $oi<'a> {
            $($ei(MaybeBorrowMut<'a, $et>),)*
        }

        #[allow(clippy::enum_variant_names)]
        pub(crate) enum $oir<'a> {
            $($ei(&'a $et),)*
        }

        impl<'a> $oi<'a> {
            fn from_item(item: Item) -> Option<Self> {
                match item {
                    $(Item::$ei(v) => Some(Self::$ei(v.into())),)*
                    _ => None,
                }
            }

            fn from_item_mut(item: &'a mut Item) -> Option<Self> {
                match item {
                    $(Item::$ei(v) => Some(Self::$ei(v.into())),)*
                    _ => None,
                }
            }
        }

        impl<'a> $oir<'a> {
            fn from_item(item: &'a Item) -> Option<Self> {
                match item {
                    $(Item::$ei(v) => Some(Self::$ei(v)),)*
                    _ => None,
                }
            }
        }

        $(
        impl ResItem for $ot {
            type ItemOut<'a> = $oi<'a>;
            type ItemOutRef<'a> = $oir<'a>;

            #[inline(always)]
            fn from_item<'a>(item: Item) -> Option<$oi<'a>> {
                $oi::from_item(item)
            }

            #[inline(always)]
            fn from_item_ref(item: &Item) -> Option<$oir<'_>> {
                $oir::from_item(item)
            }

            #[inline(always)]
            fn from_item_mut(item: &mut Item) -> Option<$oi<'_>> {
                $oi::from_item_mut(item)
            }
        }
        )+
        )*
    };
}

item_def! {
    Desc | DescR(wasi::filesystem::types::Descriptor) {
        IsoFSNode(Box<CapWrapper>),
    },
    IOStream | IOStreamR(wasi::io::streams::InputStream, wasi::io::streams::OutputStream) {
        IsoFSAccess(Box<FileAccessor>),
        StdinSignal(Arc<StdinSignal>),
        StdoutBp(Box<StdoutBypass>),
        StderrBp(Box<StderrBypass>),
        StdoutLBuf(Box<StdoutCbLineBuffered>),
        BoxedRead(Box<dyn Send + Sync + Read>),
    },
    Readdir | ReaddirR(wasi::filesystem::types::DirectoryEntryStream) {
        IsoFSReaddir(Box<DirEntryAccessor>),
    },
    Poll | PollR(wasi::io::poll::Pollable) {
        NullPoll(NullPollable),
        StdinPoll(StdinSignalPollable),
    },
}

impl Item {
    #[inline(always)]
    pub fn to<T>(self) -> Result<T, Self>
    where
        Self: Into<Result<T, Self>>,
    {
        self.into()
    }

    #[inline(always)]
    pub fn to_ref<T>(&self) -> Result<&T, &Self>
    where
        for<'a> &'a Self: Into<Result<&'a T, &'a Self>>,
    {
        self.into()
    }

    #[inline(always)]
    pub fn to_mut<T>(&mut self) -> Result<&mut T, &mut Self>
    where
        for<'a> &'a mut Self: Into<Result<&'a mut T, &'a mut Self>>,
    {
        self.into()
    }
}

impl<'t> MaybeBorrowMut<'t, Item> {
    pub fn to<T>(self) -> Result<MaybeBorrowMut<'t, T>, Self>
    where
        Item: Into<Result<T, Item>>,
        for<'a> &'a mut Item: Into<Result<&'a mut T, &'a mut Item>>,
    {
        match self {
            Self::Owned(v) => match v.into() {
                Ok(v) => Ok(MaybeBorrowMut::Owned(v)),
                Err(v) => Err(MaybeBorrowMut::Owned(v)),
            },
            Self::Borrowed(v) => match v.into() {
                Ok(v) => Ok(MaybeBorrowMut::Borrowed(v)),
                Err(v) => Err(MaybeBorrowMut::Borrowed(v)),
            },
        }
    }
}

impl Items {
    pub(crate) const fn new() -> Self {
        Self {
            data: Vec::new(),
            next: 0,
        }
    }

    pub(crate) fn get(&self, i: usize) -> Option<&Item> {
        match self.data.get(i)? {
            MaybeItem::Item(v) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn get_mut(&mut self, i: usize) -> Option<&mut Item> {
        match self.data.get_mut(i)? {
            MaybeItem::Item(v) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn remove(&mut self, i: usize) -> Option<Item> {
        let v = self.data.get_mut(i)?;
        match replace(v, MaybeItem::Empty(self.next)) {
            MaybeItem::Item(v) => {
                self.next = i;
                Some(v)
            }
            t @ MaybeItem::Empty(_) => {
                *v = t;
                None
            }
        }
    }

    pub(crate) fn insert(&mut self, v: Item) -> usize {
        if let Some(t) = self.data.get_mut(self.next) {
            let i = self.next;
            let MaybeItem::Empty(j) = *t else {
                unreachable!("Slot {i} should be empty")
            };
            (*t, self.next) = (MaybeItem::Item(v), j);
            i
        } else {
            let i = self.data.len();
            self.data.push(MaybeItem::Item(v));
            self.next = self.data.len();
            i
        }
    }

    #[inline(always)]
    pub(crate) fn get_item<T: GetItem>(&mut self, t: T) -> AnyResult<T::Output<'_>> {
        t.get_item(self)
    }

    #[inline(always)]
    pub(crate) fn get_item_ref<'a, T: GetItem>(&'a self, t: &T) -> AnyResult<T::OutputRef<'a>> {
        t.get_item_ref(self)
    }

    #[inline(always)]
    pub(crate) fn maybe_unregister<T: GetItem>(&mut self, t: T) {
        t.maybe_unregister(self)
    }
}

pub(crate) trait ResItem {
    type ItemOut<'a>;
    type ItemOutRef<'a>;

    fn from_item<'a>(item: Item) -> Option<Self::ItemOut<'a>>;
    fn from_item_ref(item: &Item) -> Option<Self::ItemOutRef<'_>>;
    fn from_item_mut(item: &mut Item) -> Option<Self::ItemOut<'_>>;
}

pub(crate) trait GetItem {
    type Output<'a>;
    type OutputRef<'a>;

    fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>>;
    fn get_item_ref<'a>(&self, items: &'a Items) -> AnyResult<Self::OutputRef<'a>>;
    fn maybe_unregister(self, items: &mut Items);
}

pub enum MaybeBorrowMut<'a, T> {
    Owned(T),
    Borrowed(&'a mut T),
}

impl<T> From<T> for MaybeBorrowMut<'_, T> {
    fn from(v: T) -> Self {
        Self::Owned(v)
    }
}

impl<'a, T> From<&'a mut T> for MaybeBorrowMut<'a, T> {
    fn from(v: &'a mut T) -> Self {
        Self::Borrowed(v)
    }
}

impl<T> Deref for MaybeBorrowMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Self::Owned(v) => v,
            Self::Borrowed(v) => v,
        }
    }
}

impl<T> DerefMut for MaybeBorrowMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        match self {
            Self::Owned(v) => v,
            Self::Borrowed(v) => v,
        }
    }
}

impl<T: ResItem + 'static> GetItem for Resource<T> {
    type Output<'a> = T::ItemOut<'a>;
    type OutputRef<'a> = T::ItemOutRef<'a>;

    fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>> {
        if self.owned() {
            items
                .remove(self.rep().try_into()?)
                .and_then(T::from_item)
                .ok_or_else(|| errors::InvalidResourceIDError::from_iter([self.rep()]).into())
        } else {
            items
                .get_mut(self.rep().try_into()?)
                .and_then(T::from_item_mut)
                .ok_or_else(|| errors::InvalidResourceIDError::from_iter([self.rep()]).into())
        }
    }

    fn get_item_ref<'a>(&self, items: &'a Items) -> AnyResult<Self::OutputRef<'a>> {
        items
            .get(self.rep().try_into()?)
            .and_then(T::from_item_ref)
            .ok_or_else(|| errors::InvalidResourceIDError::from_iter([self.rep()]).into())
    }

    fn maybe_unregister(self, items: &mut Items) {
        if self.owned() {
            let Ok(i) = usize::try_from(self.rep()) else {
                return;
            };
            items.remove(i);
        }
    }
}

macro_rules! impl_getitem_tuple {
    (#tuple $($t:ident),+) => {
        impl<$($t: ResItem + 'static),+> GetItem for ($(Resource<$t>),+) {
            type Output<'a> = ($($t::ItemOut<'a>),+);
            type OutputRef<'a> = ($($t::ItemOutRef<'a>),+);

            #[allow(non_snake_case)]
            fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>> {
                let mut errval = errors::InvalidResourceIDError::default();
                let ($($t),+) = self;

                // Check for duplicates.
                {
                    let arr = [$($t.rep()),+];
                    for (ix, &i) in arr.iter().enumerate() {
                        for &j in &arr[ix + 1..] {
                            if i == j {
                                errval.extend([i]);
                            }
                        }
                    }
                    if !errval.is_empty() {
                        return Err(errval.into());
                    }
                }

                $(
                // SAFETY: Slab remove does not move other elements.
                let $t = unsafe {
                    let ix = usize::try_from($t.rep()).ok();
                    let temp = if $t.owned() {
                        ix.and_then(|i| items.remove(i)).and_then($t::from_item)
                    } else {
                        ix.and_then(|i| items.get_mut(i)).and_then(|v| $t::from_item_mut(&mut *(&raw mut *v)))
                    };
                    if temp.is_none() {
                        errval.extend([$t.rep()]);
                    }
                    temp
                };
                )+

                match ($($t),+) {
                    ($(Some($t)),+) => Ok(($($t),+)),
                    _ => Err(errval.into()),
                }
            }

            #[allow(non_snake_case)]
            fn get_item_ref<'a>(&self, items: &'a Items) -> AnyResult<Self::OutputRef<'a>> {
                let mut errval = errors::InvalidResourceIDError::default();
                let ($($t),+) = self;

                $(
                let $t = {
                    let temp = usize::try_from($t.rep()).ok().and_then(|i| items.get(i)).and_then($t::from_item_ref);
                    if temp.is_none() {
                        errval.extend([$t.rep()]);
                    }
                    temp
                };
                )+

                match ($($t),+) {
                    ($(Some($t)),+) => Ok(($($t),+)),
                    _ => Err(errval.into()),
                }
            }

            #[allow(non_snake_case)]
            fn maybe_unregister(self, items: &mut Items) {
                let ($($t),+) = self;

                $(
                if $t.owned() {
                    if let Ok(ix) = usize::try_from($t.rep()) {
                        items.remove(ix);
                    }
                }
                )+
            }
        }
    };
    ($r:ident $(,$t:ident)*) => {
        impl_getitem_tuple!{#tuple $r $(,$t)*}
        impl_getitem_tuple!{#tuple $($t),*}
    };
}

impl_getitem_tuple! {
    A, B, C, D,
    E, F, G, H,
    I, J, K, L,
    M, N, O, P,
    Q, R, S, T,
    U, V, W, X,
    Y, Z, Aa, Ab,
    Ac, Ad, Ae, Af
}

impl<T: ResItem + 'static> GetItem for Vec<Resource<T>> {
    type Output<'a> = Vec<T::ItemOut<'a>>;
    type OutputRef<'a> = Vec<T::ItemOutRef<'a>>;

    fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>> {
        let mut errval = errors::InvalidResourceIDError::default();

        // Check for duplicates.
        for (ix, i) in self.iter().enumerate() {
            for j in &self[ix + 1..] {
                if i.rep() == j.rep() {
                    errval.extend([i.rep()]);
                }
            }
        }
        if !errval.is_empty() {
            return Err(errval.into());
        }

        let mut ret = Vec::with_capacity(self.len());
        for r in self.into_iter() {
            let ix = usize::try_from(r.rep()).ok();
            let v = if r.owned() {
                ix.and_then(|i| items.remove(i)).and_then(T::from_item)
            } else {
                ix.and_then(|i| items.get_mut(i)).and_then(|v| {
                    // SAFETY: Slab remove does not move other elements.
                    #[allow(clippy::deref_addrof)]
                    unsafe {
                        T::from_item_mut(&mut *(&raw mut *v))
                    }
                })
            };

            let Some(v) = v else {
                errval.extend([r.rep()]);
                continue;
            };
            if errval.is_empty() {
                ret.push(v);
            }
        }

        if errval.is_empty() {
            Ok(ret)
        } else {
            Err(errval.into())
        }
    }

    fn get_item_ref<'a>(&self, items: &'a Items) -> AnyResult<Self::OutputRef<'a>> {
        let mut errval = errors::InvalidResourceIDError::default();
        let mut ret = Vec::with_capacity(self.len());
        for r in self.iter() {
            let v = usize::try_from(r.rep())
                .ok()
                .and_then(|i| items.get(i))
                .and_then(T::from_item_ref);

            let Some(v) = v else {
                errval.extend([r.rep()]);
                continue;
            };
            if errval.is_empty() {
                ret.push(v);
            }
        }

        if errval.is_empty() {
            Ok(ret)
        } else {
            Err(errval.into())
        }
    }

    fn maybe_unregister(self, items: &mut Items) {
        for r in self.into_iter() {
            if r.owned() {
                let Ok(i) = usize::try_from(r.rep()) else {
                    continue;
                };
                items.remove(i);
            }
        }
    }
}
