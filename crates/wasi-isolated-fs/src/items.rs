use std::borrow::{Borrow, BorrowMut};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::Read;
use std::mem::replace;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use anyhow::Result as AnyResult;
use wasmtime::component::Resource;

use crate::bindings::wasi;
use crate::clock::ClockPollable;
use crate::errors;
use crate::fs_host::{CapWrapper as HostCapWrapper, FileStream, ReadDir as HostReadDir};
use crate::fs_isolated::{CapWrapper, DirEntryAccessor, FileAccessor};
use crate::stdio::{
    NullStdio, StderrBypass, StdinSignal, StdinSignalPollable, StdoutBypass, StdoutCbBlockBuffered,
    StdoutCbLineBuffered,
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

impl Debug for Items {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_map()
            .entries(self.data.iter().enumerate().filter_map(|(i, v)| match v {
                MaybeItem::Item(v) => Some((i, v)),
                _ => None,
            }))
            .finish()
    }
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

        impl TryFrom<Item> for $et {
            type Error = Item;

            fn try_from(v: Item) -> Result<Self, Item> {
                match v {
                    Item::$ei(v) => Ok(v),
                    v => Err(v),
                }
            }
        }

        impl<'a> TryFrom<&'a Item> for &'a $et {
            type Error = &'a Item;

            fn try_from(v: &'a Item) -> Result<Self, &'a Item> {
                match v {
                    Item::$ei(v) => Ok(v),
                    v => Err(v),
                }
            }
        }

        impl<'a> TryFrom<&'a mut Item> for &'a mut $et {
            type Error = &'a mut Item;

            fn try_from(v: &'a mut Item) -> Result<Self, &'a mut Item> {
                match v {
                    Item::$ei(v) => Ok(v),
                    v => Err(v),
                }
            }
        }
        )*

        #[allow(clippy::enum_variant_names, dead_code)]
        pub(crate) enum $oi<'a> {
            $($ei(MaybeBorrowMut<'a, $et>),)*
        }

        #[allow(clippy::enum_variant_names, dead_code)]
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
        impl ResItem for Resource<$ot> {
            type ItemOut<'a> = $oi<'a>;
            type ItemOutRef<'a> = $oir<'a>;

            #[inline(always)]
            fn is_owned(&self) -> bool {
                self.owned()
            }

            #[inline(always)]
            fn id(&self) -> u32 {
                self.rep()
            }

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
        HostFSDesc(Box<HostCapWrapper>),
    },
    IOStream | IOStreamR(wasi::io::streams::InputStream, wasi::io::streams::OutputStream) {
        IsoFSAccess(Box<FileAccessor>),
        HostFSStream(Box<FileStream>),
        StdinSignal(Arc<StdinSignal>),
        StdoutBp(Arc<StdoutBypass>),
        StderrBp(Arc<StderrBypass>),
        StdoutLBuf(Arc<StdoutCbLineBuffered>),
        StdoutBBuf(Arc<StdoutCbBlockBuffered>),
        BoxedRead(Box<dyn Send + Sync + Read>),
        NullStdio(NullStdio),
    },
    Readdir | ReaddirR(wasi::filesystem::types::DirectoryEntryStream) {
        IsoFSReaddir(Box<DirEntryAccessor>),
        HostFSReaddir(Box<HostReadDir>),
    },
    Poll | PollR(wasi::io::poll::Pollable) {
        NullPoll(NullPollable),
        StdinPoll(StdinSignalPollable),
        ClockPoll(Box<ClockPollable>),
    },
}

impl Debug for Item {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::IsoFSNode(v) => f.debug_tuple("Item::IsoFSNode").field(v).finish(),
            Self::HostFSDesc(v) => f.debug_tuple("Item::HostFSDesc").field(v).finish(),
            Self::IsoFSAccess(v) => f.debug_tuple("Item::IsoFSAccess").field(v).finish(),
            Self::HostFSStream(v) => f.debug_tuple("Item::HostFSStream").field(v).finish(),
            Self::StdinSignal(v) => f.debug_tuple("Item::StdinSignal").field(v).finish(),
            Self::StdoutBp(v) => f.debug_tuple("Item::StdoutBp").field(v).finish(),
            Self::StderrBp(v) => f.debug_tuple("Item::StderrBp").field(v).finish(),
            Self::StdoutLBuf(v) => f.debug_tuple("Item::StdoutLBuf").field(v).finish(),
            Self::StdoutBBuf(v) => f.debug_tuple("Item::StdoutBBuf").field(v).finish(),
            Self::BoxedRead(v) => f
                .debug_tuple("Item::BoxedRead")
                .field(&(&**v as *const _))
                .finish(),
            Self::NullStdio(v) => f.debug_tuple("Item::NullStdio").field(v).finish(),
            Self::IsoFSReaddir(v) => f.debug_tuple("Item::IsoFSReaddir").field(v).finish(),
            Self::HostFSReaddir(v) => f.debug_tuple("Item::HostFSReaddir").field(v).finish(),
            Self::NullPoll(v) => f.debug_tuple("Item::NullPoll").field(v).finish(),
            Self::StdinPoll(v) => f.debug_tuple("Item::StdinPoll").field(v).finish(),
            Self::ClockPoll(v) => f.debug_tuple("Item::ClockPoll").field(v).finish(),
        }
    }
}

impl<'t> MaybeBorrowMut<'t, Item> {
    pub fn to<T>(self) -> Result<MaybeBorrowMut<'t, T>, Self>
    where
        T: TryFrom<Item, Error = Item>,
        for<'a> &'a mut T: TryFrom<&'a mut Item, Error = &'a mut Item>,
    {
        match self {
            Self::Owned(v) => match v.try_into() {
                Ok(v) => Ok(MaybeBorrowMut::Owned(v)),
                Err(v) => Err(MaybeBorrowMut::Owned(v)),
            },
            Self::Borrowed(v) => match v.try_into() {
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

    fn is_owned(&self) -> bool;
    fn id(&self) -> u32;
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

impl<T> Borrow<T> for MaybeBorrowMut<'_, T> {
    fn borrow(&self) -> &T {
        match self {
            Self::Owned(v) => v,
            Self::Borrowed(v) => v,
        }
    }
}

impl<T> BorrowMut<T> for MaybeBorrowMut<'_, T> {
    fn borrow_mut(&mut self) -> &mut T {
        match self {
            Self::Owned(v) => v,
            Self::Borrowed(v) => v,
        }
    }
}

impl<T: Debug> Debug for MaybeBorrowMut<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
    }
}

impl<T: ResItem + 'static> GetItem for T {
    type Output<'a> = T::ItemOut<'a>;
    type OutputRef<'a> = T::ItemOutRef<'a>;

    fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>> {
        if self.is_owned() {
            items
                .remove(self.id().try_into()?)
                .and_then(T::from_item)
                .ok_or_else(|| errors::InvalidResourceIDError::from_iter([self.id()]).into())
        } else {
            items
                .get_mut(self.id().try_into()?)
                .and_then(T::from_item_mut)
                .ok_or_else(|| errors::InvalidResourceIDError::from_iter([self.id()]).into())
        }
    }

    fn get_item_ref<'a>(&self, items: &'a Items) -> AnyResult<Self::OutputRef<'a>> {
        items
            .get(self.id().try_into()?)
            .and_then(T::from_item_ref)
            .ok_or_else(|| errors::InvalidResourceIDError::from_iter([self.id()]).into())
    }

    fn maybe_unregister(self, items: &mut Items) {
        if self.is_owned() {
            if let Ok(i) = usize::try_from(self.id()) {
                items.remove(i);
            }
        }
    }
}

macro_rules! impl_getitem_tuple {
    (#tuple $($t:ident),+) => {
        impl<$($t: ResItem + 'static),+> GetItem for ($($t,)+) {
            type Output<'a> = ($($t::ItemOut<'a>,)+);
            type OutputRef<'a> = ($($t::ItemOutRef<'a>,)+);

            #[allow(non_snake_case)]
            fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>> {
                let mut errval = errors::InvalidResourceIDError::default();
                let ($($t,)+) = self;

                // Check for duplicates.
                {
                    let arr = [$($t.id()),+];
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
                    let ix = usize::try_from($t.id()).ok();
                    let temp = if $t.is_owned() {
                        ix.and_then(|i| items.remove(i)).and_then($t::from_item)
                    } else {
                        ix.and_then(|i| items.get_mut(i)).and_then(|v| $t::from_item_mut(&mut *(&raw mut *v)))
                    };
                    if temp.is_none() {
                        errval.extend([$t.id()]);
                    }
                    temp
                };
                )+

                match ($($t,)+) {
                    ($(Some($t),)+) => Ok(($($t,)+)),
                    _ => Err(errval.into()),
                }
            }

            #[allow(non_snake_case)]
            fn get_item_ref<'a>(&self, items: &'a Items) -> AnyResult<Self::OutputRef<'a>> {
                let mut errval = errors::InvalidResourceIDError::default();
                let ($($t,)+) = self;

                $(
                let $t = {
                    let temp = usize::try_from($t.id()).ok().and_then(|i| items.get(i)).and_then($t::from_item_ref);
                    if temp.is_none() {
                        errval.extend([$t.id()]);
                    }
                    temp
                };
                )+

                match ($($t,)+) {
                    ($(Some($t),)+) => Ok(($($t,)+)),
                    _ => Err(errval.into()),
                }
            }

            #[allow(non_snake_case)]
            fn maybe_unregister(self, items: &mut Items) {
                let ($($t,)+) = self;

                $(
                if $t.is_owned() {
                    if let Ok(ix) = usize::try_from($t.id()) {
                        items.remove(ix);
                    }
                }
                )+
            }
        }
    };
    () => {};
    ($r:ident $(,$t:ident)*) => {
        impl_getitem_tuple!{#tuple $r $(,$t)*}
        impl_getitem_tuple!{$($t),*}
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

impl<T: ResItem + 'static> GetItem for Vec<T> {
    type Output<'a> = Vec<T::ItemOut<'a>>;
    type OutputRef<'a> = Vec<T::ItemOutRef<'a>>;

    fn get_item(self, items: &mut Items) -> AnyResult<Self::Output<'_>> {
        let mut errval = errors::InvalidResourceIDError::default();

        // Check for duplicates.
        for (ix, i) in self.iter().enumerate() {
            for j in &self[ix + 1..] {
                if i.id() == j.id() {
                    errval.extend([i.id()]);
                }
            }
        }
        if !errval.is_empty() {
            return Err(errval.into());
        }

        let mut ret = Vec::with_capacity(self.len());
        for r in self.into_iter() {
            let ix = usize::try_from(r.id()).ok();
            let v = if r.is_owned() {
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
                errval.extend([r.id()]);
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
            let v = usize::try_from(r.id())
                .ok()
                .and_then(|i| items.get(i))
                .and_then(T::from_item_ref);

            let Some(v) = v else {
                errval.extend([r.id()]);
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
            if r.is_owned() {
                if let Ok(i) = usize::try_from(r.id()) {
                    items.remove(i);
                }
            }
        }
    }
}
