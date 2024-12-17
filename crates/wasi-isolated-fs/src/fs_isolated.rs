use std::collections::btree_map::{BTreeMap, Entry};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::{ErrorKind, SeekFrom};
use std::mem::replace;
use std::ops::{BitAnd, Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use anyhow::{Error, Result as AnyResult};
use camino::{Utf8Component, Utf8Path};
use parking_lot::{Mutex, MutexGuard, RwLock, RwLockWriteGuard};
use smallvec::SmallVec;

use crate::bindings::wasi;
use crate::errors;

pub const LINK_DEPTH: usize = 10;

pub(crate) static ILLEGAL_CHARS: &[char] = &['\\', '/', ':', '*', '?', '\"', '\'', '<', '>', '|'];

pub struct IsolatedFSController {
    limits: Arc<FSLimits>,
    root: Arc<Node>,

    state: RandomState,
}

impl IsolatedFSController {
    pub fn new(max_size: usize, max_node: usize) -> AnyResult<Self> {
        let limits = Arc::new(FSLimits::new(max_size, max_node));
        if !limits.take_node(1) {
            return Err(errors::FileLimitError::Node.into());
        }

        Ok(Self {
            root: Arc::new_cyclic(|this| {
                Node::from((
                    Dir {
                        limits: AcqNode {
                            limits: Arc::downgrade(&limits),
                            inode: limits.get_inode(),
                        },
                        stamp: Timestamp::new(),

                        items: BTreeMap::new(),
                    },
                    this.clone(),
                ))
            }),
            limits,

            state: RandomState::new(),
        })
    }

    #[inline(always)]
    pub fn root(&self) -> Arc<Node> {
        self.root.clone()
    }

    pub(crate) fn dup(&self) -> Self {
        Self {
            limits: self.limits.clone(),
            root: self.root.clone(),

            state: RandomState::new(),
        }
    }
}

struct FSLimits {
    cur_size: AtomicUsize,
    cur_node: AtomicUsize,
    inode: AtomicUsize,
}

impl FSLimits {
    fn new(max_size: usize, max_node: usize) -> Self {
        Self {
            cur_size: AtomicUsize::new(max_size),
            cur_node: AtomicUsize::new(max_node),
            inode: AtomicUsize::new(0),
        }
    }

    fn take_val(cur: &AtomicUsize, num: usize) -> bool {
        if num == 0 {
            return true;
        }

        let mut s = cur.load(Ordering::Acquire);
        loop {
            let Some(n) = s.checked_sub(num) else {
                break false;
            };
            s = match cur.compare_exchange_weak(s, n, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => break true,
                Err(v) => v,
            };
        }
    }

    fn put_val(cur: &AtomicUsize, num: usize) {
        if num > 0 {
            cur.fetch_add(num, Ordering::Relaxed);
        }
    }

    fn get_inode(&self) -> usize {
        self.inode.fetch_add(1, Ordering::SeqCst)
    }

    fn take_size(&self, size: usize) -> bool {
        Self::take_val(&self.cur_size, size)
    }

    fn take_node(&self, size: usize) -> bool {
        Self::take_val(&self.cur_node, size)
    }

    fn weak_take_size(this: &Weak<Self>, size: usize) -> bool {
        match this.upgrade() {
            Some(v) => v.take_size(size),
            None => false,
        }
    }

    fn put_size_node(this: &Weak<Self>, size: usize, node: usize) {
        if let Some(v) = this.upgrade() {
            Self::put_val(&v.cur_size, size);
            Self::put_val(&v.cur_node, node);
        }
    }
}

struct AcqNode {
    limits: Weak<FSLimits>,
    inode: usize,
}

impl Drop for AcqNode {
    fn drop(&mut self) {
        FSLimits::put_size_node(&self.limits, 0, 1);
    }
}

impl AcqNode {
    fn new(controller: &IsolatedFSController) -> AnyResult<Self> {
        if !controller.limits.take_node(1) {
            Err(errors::FileLimitError::Node.into())
        } else {
            Ok(Self {
                limits: Arc::downgrade(&controller.limits),
                inode: controller.limits.get_inode(),
            })
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Timestamp {
    pub ctime: SystemTime,
    pub mtime: SystemTime,
    pub atime: SystemTime,
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::new()
    }
}

impl Timestamp {
    pub fn new() -> Self {
        let t = SystemTime::now();
        Self {
            ctime: t,
            mtime: t,
            atime: t,
        }
    }

    pub fn access(&mut self) {
        self.atime = SystemTime::now();
    }

    pub fn modify(&mut self) {
        let t = SystemTime::now();
        self.mtime = t;
        self.atime = t;
    }
}

type FileChunk = SmallVec<[u8; 16]>;

pub struct File {
    limits: Weak<FSLimits>,
    inode: usize,
    stamp: Timestamp,

    size: usize,
    size_chunks: usize,
    data: SmallVec<[FileChunk; 4]>,
}

impl Drop for File {
    fn drop(&mut self) {
        FSLimits::put_size_node(&self.limits, self.size_chunks, 1);
    }
}

impl File {
    pub fn new(controller: &IsolatedFSController) -> AnyResult<Self> {
        if !controller.limits.take_node(1) {
            return Err(errors::FileLimitError::Node.into());
        }

        Ok(Self {
            limits: Arc::downgrade(&controller.limits),
            inode: controller.limits.get_inode(),
            stamp: Timestamp::new(),

            size: 0,
            size_chunks: 0,
            data: Default::default(),
        })
    }

    #[inline(always)]
    pub(crate) fn inode(&self) -> usize {
        self.inode
    }

    #[inline(always)]
    pub fn stamp(&self) -> &Timestamp {
        &self.stamp
    }

    #[inline(always)]
    pub fn stamp_mut(&mut self) -> &mut Timestamp {
        &mut self.stamp
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.size
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.size_chunks
    }

    pub fn read(&mut self, len: usize, off: usize) -> (&[u8], usize) {
        let ret: (&[_], _) = if len == 0 || off >= self.size {
            (&[], 0)
        } else if let Some(v) = self.data.get(off >> 16) {
            let o = off & 65535;
            let e = o
                .saturating_add(len)
                .min(self.size - (off & !65535))
                .min(65536);
            let l = e - o;
            (
                v.get(o..).map_or(&[] as &[_], |v| v.get(..l).unwrap_or(v)),
                l,
            )
        } else {
            (&[], 0)
        };

        self.stamp.access();
        ret
    }

    pub fn write(&mut self, mut buf: &[u8], off: usize) -> AnyResult<()> {
        if buf.is_empty() {
            return Ok(());
        }

        let end = off + buf.len();
        if end > self.size {
            let ec = (end & !65535) + Self::clamped_size(end & 65535);
            let v = ec.saturating_sub(self.size_chunks);
            if v > 0 {
                if !FSLimits::weak_take_size(&self.limits, v) {
                    return Err(errors::FileLimitError::Size(v).into());
                }
                self.size_chunks = ec;
            }

            self.size = end;
        }

        self.stamp.modify();
        let (mut d, mut r) = (off >> 16, off & 65535);
        while !buf.is_empty() {
            let Some(v) = self.data.get_mut(d) else {
                self.data.push(FileChunk::from_buf(Default::default()));
                continue;
            };

            let s = r.saturating_add(buf.len()).min(65536);
            if s > v.len() && s > 16 {
                let s = Self::clamped_size(s);
                v.reserve_exact(s - v.len());
                v.resize(s, 0);
            }

            let (a, b) = buf.split_at(s);
            v[r..s].copy_from_slice(&a[r..]);
            (buf, d, r) = (b, d + 1, 0);
        }

        Ok(())
    }

    pub fn resize(&mut self, size: usize) -> AnyResult<()> {
        if size <= self.size {
            self.truncate(size);
            return Ok(());
        }
        self.stamp.modify();

        let ec = (size & !65535) + Self::clamped_size(size & 65535);
        let v = ec.saturating_sub(self.size_chunks);
        if v > 0 {
            if !FSLimits::weak_take_size(&self.limits, v) {
                return Err(errors::FileLimitError::Size(v).into());
            }
            self.size_chunks = ec;
        }

        for _ in self.size >> 16..size >> 16 {
            self.data.push(FileChunk::from_buf(Default::default()));
        }
        self.size = size;

        Ok(())
    }

    pub fn truncate(&mut self, size: usize) {
        self.stamp.modify();
        if size >= self.size {
            return;
        }

        let new_chunks = size.saturating_add(65535) & !65535;
        let v = self.size_chunks.saturating_sub(new_chunks);
        if v > 0 {
            FSLimits::put_size_node(&self.limits, v, 0);
            self.size_chunks = new_chunks;
        }
        self.size = size;
        self.data.truncate(new_chunks >> 16);
    }

    /// Clamped chunk size.
    fn clamped_size(v: usize) -> usize {
        if v == 0 {
            0
        } else {
            v.checked_next_power_of_two()
                .unwrap_or(usize::MAX)
                .clamp(4096, 65536)
        }
    }
}

pub struct Dir {
    limits: AcqNode,
    stamp: Timestamp,

    pub(crate) items: BTreeMap<Arc<str>, Arc<Node>>,
}

impl Dir {
    pub fn new(controller: &IsolatedFSController) -> AnyResult<Self> {
        Ok(Self {
            limits: AcqNode::new(controller)?,
            stamp: Timestamp::new(),

            items: BTreeMap::new(),
        })
    }

    #[inline(always)]
    pub(crate) fn inode(&self) -> usize {
        self.limits.inode
    }

    #[inline(always)]
    pub fn stamp(&self) -> &Timestamp {
        &self.stamp
    }

    #[inline(always)]
    pub fn stamp_mut(&mut self) -> &mut Timestamp {
        &mut self.stamp
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&mut self, key: impl AsRef<str>) -> Option<Arc<Node>> {
        self.stamp.access();
        self.items.get(key.as_ref()).cloned()
    }

    pub fn add<E>(
        &mut self,
        key: impl Into<Arc<str>>,
        f: impl FnOnce() -> Result<Arc<Node>, E>,
    ) -> Result<Option<Arc<Node>>, E> {
        Ok(match self.items.entry(key.into()) {
            Entry::Vacant(v) => {
                self.stamp.modify();
                let f = f()?;
                v.insert(f.clone());
                Some(f)
            }
            Entry::Occupied(_) => None,
        })
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let r = self.items.remove(key).is_some();
        if r {
            self.stamp.modify();
        }

        r
    }

    pub fn iter(&self) -> impl use<'_> + Iterator<Item = (&'_ str, &'_ Arc<Node>)> {
        self.items.iter().map(|(k, v)| (&**k, v))
    }
}

type LinkSegmentType = SmallVec<[usize; 4]>;

pub struct Link {
    limits: AcqNode,
    stamp: Timestamp,

    path: String,
    segments: LinkSegmentType,
    len: usize,
}

impl Link {
    fn gen_link(str_: &mut String, seg: &mut LinkSegmentType, len: &mut usize, path: &Utf8Path) {
        for c in path.components() {
            match c {
                Utf8Component::CurDir => (),
                Utf8Component::ParentDir => {
                    if let Some(&i) = seg.last() {
                        match &str_[i..] {
                            "/" => continue,
                            "." => (),
                            s => {
                                *len -= s.len() + 1;
                                seg.pop();
                                str_.truncate(i);
                                continue;
                            }
                        }
                    }

                    *len += if seg.is_empty() { 2 } else { 3 };
                    seg.push(str_.len());
                    *str_ += ".";
                }
                Utf8Component::Normal(s) => {
                    seg.push(str_.len());
                    *str_ += s;
                    *len += s.len() + 1;
                }
                Utf8Component::RootDir | Utf8Component::Prefix(_) => {
                    str_.clear();
                    seg.clear();

                    *str_ += "/";
                    seg.push(0);
                    *len = 0;
                }
            };
        }

        *len = (*len).max(1);
    }

    pub fn new(controller: &IsolatedFSController, path: &Utf8Path) -> AnyResult<Self> {
        let limits = AcqNode::new(controller)?;
        let mut p = String::with_capacity(path.as_str().len());
        let mut segments = LinkSegmentType::new();
        let mut len = 1;

        Self::gen_link(&mut p, &mut segments, &mut len, path);

        Ok(Self {
            limits,
            stamp: Timestamp::new(),

            path: p,
            segments,
            len,
        })
    }

    #[inline(always)]
    pub(crate) fn inode(&self) -> usize {
        self.limits.inode
    }

    #[inline(always)]
    pub fn stamp(&self) -> &Timestamp {
        &self.stamp
    }

    #[inline(always)]
    pub fn stamp_mut(&mut self) -> &mut Timestamp {
        &mut self.stamp
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl use<'_> + Iterator<Item = Utf8Component<'_>> + Send {
        enum Iter<'a> {
            CurDir,
            Walk { dir: &'a Link, ix: usize },
            End,
        }

        impl<'a> Iterator for Iter<'a> {
            type Item = Utf8Component<'a>;

            fn next(&mut self) -> Option<Self::Item> {
                match replace(self, Self::End) {
                    Self::End => None,
                    Self::CurDir => Some(Utf8Component::CurDir),
                    Self::Walk { dir, ix } => {
                        let i = dir.segments[ix];
                        let s = if let Some(&e) = dir.segments.get(ix + 1) {
                            *self = Self::Walk { dir, ix: ix + 1 };
                            &dir.path[i..e]
                        } else {
                            &dir.path[i..]
                        };

                        Some(match s {
                            "/" => Utf8Component::RootDir,
                            "." => Utf8Component::ParentDir,
                            s => Utf8Component::Normal(s),
                        })
                    }
                }
            }
        }

        if self.segments.is_empty() {
            Iter::CurDir
        } else {
            Iter::Walk { dir: self, ix: 0 }
        }
    }

    pub fn get(&self) -> String {
        let mut ret = String::with_capacity(self.len);

        for ix in 0..self.segments.len() {
            let i = self.segments[ix];
            let s = match self.segments.get(ix + 1) {
                Some(&j) => &self.path[i..j],
                None => &self.path[i..],
            };

            match s {
                "/" => {
                    ret.clear();
                    ret += "/";
                }
                "." => ret += if ret.is_empty() { ".." } else { "/.." },
                s if ret.is_empty() => ret += s,
                s => ret.extend(["/", s]),
            }
        }
        if ret.is_empty() {
            ret += ".";
        }

        ret
    }

    pub fn set(&mut self, path: &Utf8Path) {
        self.path.clear();
        self.segments.clear();
        self.len = 1;
        self.path.reserve(path.as_str().len());

        Self::gen_link(&mut self.path, &mut self.segments, &mut self.len, path);
        self.stamp.modify();
    }
}

pub(crate) enum NodeItem {
    File(Mutex<File>),
    Dir(Mutex<Dir>),
    Link(RwLock<Link>),
}

pub struct Node(pub(crate) NodeItem, RwLock<Weak<Node>>);

impl Node {
    fn node_ty(&self) -> errors::NodeItemTy {
        match self.0 {
            NodeItem::Dir(_) => errors::NodeItemTy::Dir,
            NodeItem::File(_) => errors::NodeItemTy::File,
            NodeItem::Link(_) => errors::NodeItemTy::Link,
        }
    }

    #[inline(always)]
    pub fn is_dir(&self) -> bool {
        matches!(self.0, NodeItem::Dir(_))
    }

    #[inline(always)]
    pub fn is_file(&self) -> bool {
        matches!(self.0, NodeItem::File(_))
    }

    #[inline(always)]
    pub fn is_link(&self) -> bool {
        matches!(self.0, NodeItem::Link(_))
    }

    pub fn dir(&self) -> Option<impl '_ + DerefMut<Target = Dir>> {
        match &self.0 {
            NodeItem::Dir(v) => Some(v.lock()),
            _ => None,
        }
    }

    pub fn file(&self) -> Option<impl '_ + DerefMut<Target = File>> {
        match &self.0 {
            NodeItem::File(v) => Some(v.lock()),
            _ => None,
        }
    }

    pub fn link(&self) -> Option<impl '_ + DerefMut<Target = Link>> {
        match &self.0 {
            NodeItem::Link(v) => Some(v.write()),
            _ => None,
        }
    }

    pub fn try_dir(&self) -> AnyResult<impl '_ + DerefMut<Target = Dir>> {
        match &self.0 {
            NodeItem::Dir(v) => Ok(v.lock()),
            _ => Err(errors::WrongNodeItemError {
                exp: errors::NodeItemTy::Dir,
                ty: self.node_ty(),
            }
            .into()),
        }
    }

    pub fn try_file(&self) -> AnyResult<impl '_ + DerefMut<Target = File>> {
        match &self.0 {
            NodeItem::File(v) => Ok(v.lock()),
            _ => Err(errors::WrongNodeItemError {
                exp: errors::NodeItemTy::File,
                ty: self.node_ty(),
            }
            .into()),
        }
    }

    pub fn try_link(&self) -> AnyResult<impl '_ + DerefMut<Target = Link>> {
        match &self.0 {
            NodeItem::Link(v) => Ok(v.write()),
            _ => Err(errors::WrongNodeItemError {
                exp: errors::NodeItemTy::Link,
                ty: self.node_ty(),
            }
            .into()),
        }
    }

    pub fn parent(&self) -> Option<Arc<Node>> {
        self.1.read().upgrade()
    }

    pub fn stamp(&self) -> impl '_ + DerefMut<Target = Timestamp> {
        enum NodeItemRef<'a> {
            File(MutexGuard<'a, File>),
            Dir(MutexGuard<'a, Dir>),
            Link(RwLockWriteGuard<'a, Link>),
        }

        impl Deref for NodeItemRef<'_> {
            type Target = Timestamp;

            fn deref(&self) -> &Timestamp {
                match self {
                    Self::File(v) => &v.stamp,
                    Self::Dir(v) => &v.stamp,
                    Self::Link(v) => &v.stamp,
                }
            }
        }

        impl DerefMut for NodeItemRef<'_> {
            fn deref_mut(&mut self) -> &mut Timestamp {
                match self {
                    Self::File(v) => &mut v.stamp,
                    Self::Dir(v) => &mut v.stamp,
                    Self::Link(v) => &mut v.stamp,
                }
            }
        }

        match &self.0 {
            NodeItem::Dir(v) => NodeItemRef::Dir(v.lock()),
            NodeItem::File(v) => NodeItemRef::File(v.lock()),
            NodeItem::Link(v) => NodeItemRef::Link(v.write()),
        }
    }

    fn parent_or_root(self: &Arc<Self>, controller: &IsolatedFSController) -> Option<Arc<Self>> {
        self.parent().or_else(|| {
            if Arc::ptr_eq(self, &controller.root) {
                Some(controller.root.clone())
            } else {
                None
            }
        })
    }

    fn file_type(&self) -> wasi::filesystem::types::DescriptorType {
        match self.0 {
            NodeItem::Dir(_) => wasi::filesystem::types::DescriptorType::Directory,
            NodeItem::File(_) => wasi::filesystem::types::DescriptorType::RegularFile,
            NodeItem::Link(_) => wasi::filesystem::types::DescriptorType::SymbolicLink,
        }
    }

    fn follow_link(
        self: Arc<Self>,
        controller: &IsolatedFSController,
        depth: usize,
    ) -> Result<Arc<Node>, errors::StreamError> {
        let (d, n) = match &self.0 {
            NodeItem::Link(v) => (
                depth
                    .checked_sub(1)
                    .ok_or(wasi::filesystem::types::ErrorCode::Loop)?,
                v.read(),
            ),
            _ => return Ok(self),
        };

        let mut ret = self.parent_or_root(controller).ok_or(ErrorKind::NotFound)?;
        for c in n.iter() {
            ret = match c {
                Utf8Component::Prefix(_) => return Err(ErrorKind::InvalidInput.into()),
                Utf8Component::RootDir => controller.root.clone(),
                Utf8Component::CurDir => continue,
                Utf8Component::ParentDir => {
                    ret.parent_or_root(controller).ok_or(ErrorKind::NotFound)?
                }
                Utf8Component::Normal(p) => ret
                    .follow_link(controller, d)?
                    .dir()
                    .ok_or(ErrorKind::NotADirectory)?
                    .get(p)
                    .ok_or(ErrorKind::NotFound)?,
            };
        }

        ret.follow_link(controller, d)
    }
}

impl From<(File, Weak<Node>)> for Node {
    fn from((v, p): (File, Weak<Node>)) -> Self {
        Self(NodeItem::File(Mutex::new(v)), RwLock::new(p))
    }
}

impl From<(Dir, Weak<Node>)> for Node {
    fn from((v, p): (Dir, Weak<Node>)) -> Self {
        Self(NodeItem::Dir(Mutex::new(v)), RwLock::new(p))
    }
}

impl From<(Link, Weak<Node>)> for Node {
    fn from((v, p): (Link, Weak<Node>)) -> Self {
        Self(NodeItem::Link(RwLock::new(v)), RwLock::new(p))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum AccessMode {
    NA,
    R,
    W,
    RW,
}

impl BitAnd for AccessMode {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::RW, Self::RW) => Self::RW,
            (Self::R, Self::R | Self::RW) | (Self::RW, Self::R) => Self::R,
            (Self::W, Self::W | Self::RW) | (Self::RW, Self::W) => Self::W,
            _ => Self::NA,
        }
    }
}

impl AccessMode {
    pub fn is_read(self) -> bool {
        matches!(self, Self::R | Self::RW)
    }

    pub fn is_write(self) -> bool {
        matches!(self, Self::W | Self::RW)
    }

    fn read_or_err(self) -> Result<(), errors::StreamError> {
        if self.is_read() {
            Ok(())
        } else {
            Err(ErrorKind::PermissionDenied.into())
        }
    }

    fn write_or_err(self) -> Result<(), errors::StreamError> {
        if self.is_write() {
            Ok(())
        } else {
            Err(ErrorKind::PermissionDenied.into())
        }
    }

    fn access_or_err(self) -> Result<(), errors::StreamError> {
        match self {
            Self::NA => Err(ErrorKind::PermissionDenied.into()),
            _ => Ok(()),
        }
    }
}

#[derive(Clone)]
pub struct CapWrapper {
    access: AccessMode,
    node: Arc<Node>,
}

impl CapWrapper {
    fn to_datetime(t: SystemTime) -> wasi::filesystem::types::Datetime {
        let (mut s, mut n);
        match t.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(v) => (s, n) = (v.as_secs() as i64, v.subsec_nanos()),
            Err(v) => {
                let v = v.duration();
                (s, n) = (-(v.as_secs() as i64), v.subsec_nanos());
                if n > 0 {
                    n = 1_000_000_000 - n;
                    s -= 1;
                }
            }
        }

        wasi::filesystem::types::Datetime {
            seconds: s as u64,
            nanoseconds: n,
        }
    }

    #[inline(always)]
    pub fn new(node: Arc<Node>, access: AccessMode) -> Self {
        Self { node, access }
    }

    #[inline(always)]
    pub fn node(&self) -> &Node {
        &self.node
    }

    #[inline(always)]
    pub fn access(&self) -> &AccessMode {
        &self.access
    }

    pub fn file_type(
        &self,
    ) -> Result<wasi::filesystem::types::DescriptorType, errors::StreamError> {
        Ok(self.node.file_type())
    }

    pub fn stat(&self) -> Result<wasi::filesystem::types::DescriptorStat, errors::StreamError> {
        let (size, mtime, atime) = match &self.node.0 {
            NodeItem::File(v) => {
                let v = v.lock();
                (v.len(), v.stamp.mtime, v.stamp.atime)
            }
            NodeItem::Dir(v) => {
                let v = v.lock();
                (v.len(), v.stamp.mtime, v.stamp.atime)
            }
            NodeItem::Link(v) => {
                let v = v.read();
                (v.len(), v.stamp.mtime, v.stamp.atime)
            }
        };

        let mtime = Self::to_datetime(mtime);
        let atime = Self::to_datetime(atime);

        Ok(wasi::filesystem::types::DescriptorStat {
            type_: self.node.file_type(),
            link_count: 0,
            size: size.try_into().map_err(Error::from)?,
            data_access_timestamp: Some(atime),
            data_modification_timestamp: Some(mtime),
            status_change_timestamp: Some(atime),
        })
    }

    pub fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.node, &other.node)
    }

    pub fn metadata_hash(
        &self,
        controller: &IsolatedFSController,
    ) -> wasi::filesystem::types::MetadataHashValue {
        let mut h1 = controller.state.build_hasher();
        {
            let (size, stamp, inode) = match &self.node.0 {
                NodeItem::File(v) => {
                    let v = v.lock();
                    (v.len(), v.stamp.clone(), v.inode())
                }
                NodeItem::Dir(v) => {
                    let v = v.lock();
                    (v.len(), v.stamp.clone(), v.inode())
                }
                NodeItem::Link(v) => {
                    let v = v.read();
                    (v.len(), v.stamp.clone(), v.inode())
                }
            };
            (
                Arc::as_ptr(&self.node),
                self.node.1.read().as_ptr(),
                size,
                stamp,
                inode,
            )
                .hash(&mut h1);
        }

        let mut h2 = h1.clone();
        h1.write_u32(0xc12af7ed);
        h2.write_u128(0x00265409_00274028_00288693);

        wasi::filesystem::types::MetadataHashValue {
            lower: h1.finish(),
            upper: h2.finish(),
        }
    }

    pub fn set_time(
        &self,
        mtime: SystemTime,
        atime: SystemTime,
    ) -> Result<(), errors::StreamError> {
        self.access.write_or_err()?;

        let mut stamp = self.node.stamp();
        stamp.mtime = mtime;
        stamp.atime = atime;
        Ok(())
    }

    pub fn open_file(
        &self,
        mut access: AccessMode,
        seek: SeekFrom,
    ) -> Result<FileAccessor, errors::StreamError> {
        access = self.access & access;
        access.access_or_err()?;

        match &self.node.0 {
            NodeItem::File(v) => {
                let len = v.lock().len();
                let cursor = match seek {
                    SeekFrom::Start(v) => usize::try_from(v).unwrap_or(usize::MAX),
                    SeekFrom::Current(..-1) => 0,
                    SeekFrom::Current(v) => usize::try_from(v).unwrap_or(usize::MAX),
                    SeekFrom::End(0..) => len,
                    SeekFrom::End(v) => {
                        len.saturating_sub(usize::try_from(v.wrapping_neg()).unwrap_or(usize::MAX))
                    }
                }
                .min(len);

                Ok(FileAccessor {
                    file: self.node.clone(),
                    access,
                    cursor,
                    closed: false,
                })
            }
            NodeItem::Dir(_) => Err(ErrorKind::IsADirectory.into()),
            NodeItem::Link(_) => Err(wasi::filesystem::types::ErrorCode::Loop.into()),
        }
    }

    pub fn open(
        &self,
        controller: &IsolatedFSController,
        path: &Utf8Path,
        follow_symlink: bool,
        create_file: bool,
        mut access: AccessMode,
    ) -> Result<Self, errors::StreamError> {
        access = self.access & access;
        access.access_or_err()?;

        let mut node = self.node.clone();
        let mut it = path.components().peekable();
        while let Some(c) = it.next() {
            let p = match c {
                Utf8Component::Prefix(_) => return Err(ErrorKind::InvalidInput.into()),
                Utf8Component::RootDir => {
                    node = controller.root.clone();
                    continue;
                }
                Utf8Component::CurDir => continue,
                Utf8Component::ParentDir => {
                    node = node.parent_or_root(controller).ok_or(ErrorKind::NotFound)?;
                    continue;
                }
                Utf8Component::Normal(p) if p.contains(ILLEGAL_CHARS) => {
                    return Err(ErrorKind::InvalidInput.into())
                }
                Utf8Component::Normal(p) => p,
            };

            if follow_symlink {
                node = node.follow_link(controller, LINK_DEPTH)?;
            }

            let mut v = node.dir().ok_or(ErrorKind::NotADirectory)?;
            let n = match v.get(p) {
                Some(v) => Ok(v),
                None if create_file && it.peek().is_none() => {
                    if !access.is_write() {
                        return Err(ErrorKind::PermissionDenied.into());
                    }

                    v.add::<Error>(p, || {
                        Ok(Arc::new(Node::from((
                            File::new(controller)?,
                            Arc::downgrade(&node),
                        ))))
                    })?
                    .ok_or(ErrorKind::AlreadyExists)
                }
                None => Err(ErrorKind::NotFound),
            }?;
            drop(v);
            node = n;
        }

        if follow_symlink {
            node = node.follow_link(controller, LINK_DEPTH)?;
        }

        Ok(Self { node, access })
    }

    pub fn read(&self, len: usize, off: usize) -> Result<Vec<u8>, errors::StreamError> {
        self.access.read_or_err()?;

        match &self.node.0 {
            NodeItem::File(v) => {
                let mut v = v.lock();
                let (s, l) = v.read(len, off);
                let mut ret = vec![0u8; l];
                ret[..s.len()].copy_from_slice(s);
                Ok(ret)
            }
            NodeItem::Dir(_) => Err(ErrorKind::IsADirectory.into()),
            NodeItem::Link(_) => Err(ErrorKind::InvalidInput.into()),
        }
    }

    pub fn write(&self, buf: &[u8], off: usize) -> Result<(), errors::StreamError> {
        self.access.write_or_err()?;

        match &self.node.0 {
            NodeItem::File(v) => {
                v.lock().write(buf, off)?;
                Ok(())
            }
            NodeItem::Dir(_) => Err(ErrorKind::IsADirectory.into()),
            NodeItem::Link(_) => Err(ErrorKind::InvalidInput.into()),
        }
    }

    pub fn resize(&self, size: usize) -> Result<(), errors::StreamError> {
        self.access.write_or_err()?;

        match &self.node.0 {
            NodeItem::File(v) => {
                v.lock().resize(size)?;
                Ok(())
            }
            NodeItem::Dir(_) => Err(ErrorKind::IsADirectory.into()),
            NodeItem::Link(_) => Err(ErrorKind::InvalidInput.into()),
        }
    }

    pub fn create_dir(
        &self,
        controller: &IsolatedFSController,
        name: impl Into<Arc<str>> + AsRef<str>,
    ) -> Result<Self, errors::StreamError> {
        if name.as_ref().contains(ILLEGAL_CHARS) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.access.write_or_err()?;

        Ok(Self {
            node: self
                .node
                .dir()
                .ok_or(ErrorKind::NotADirectory)?
                .add::<Error>(name, || {
                    Ok(Arc::new(Node::from((
                        Dir::new(controller)?,
                        Arc::downgrade(&self.node),
                    ))))
                })?
                .ok_or(ErrorKind::AlreadyExists)?,
            access: self.access,
        })
    }

    pub fn create_file(
        &self,
        controller: &IsolatedFSController,
        name: impl Into<Arc<str>> + AsRef<str>,
    ) -> Result<Self, errors::StreamError> {
        if name.as_ref().contains(ILLEGAL_CHARS) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.access.write_or_err()?;

        Ok(Self {
            node: self
                .node
                .dir()
                .ok_or(ErrorKind::NotADirectory)?
                .add::<Error>(name, || {
                    Ok(Arc::new(Node::from((
                        File::new(controller)?,
                        Arc::downgrade(&self.node),
                    ))))
                })?
                .ok_or(ErrorKind::AlreadyExists)?,
            access: self.access,
        })
    }

    pub fn create_link(
        &self,
        controller: &IsolatedFSController,
        name: impl Into<Arc<str>> + AsRef<str>,
        path: &Utf8Path,
    ) -> Result<Self, errors::StreamError> {
        if path.as_str().is_empty() || name.as_ref().contains(ILLEGAL_CHARS) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.access.write_or_err()?;

        Ok(Self {
            node: self
                .node
                .dir()
                .ok_or(ErrorKind::NotADirectory)?
                .add::<Error>(name, || {
                    Ok(Arc::new(Node::from((
                        Link::new(controller, path)?,
                        Arc::downgrade(&self.node),
                    ))))
                })?
                .ok_or(ErrorKind::AlreadyExists)?,
            access: self.access,
        })
    }

    pub fn move_file(
        &self,
        src: Arc<Node>,
        src_file: &str,
        dst_file: impl Into<Arc<str>> + AsRef<str>,
    ) -> Result<(), errors::StreamError> {
        if dst_file.as_ref().contains(ILLEGAL_CHARS) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.access.write_or_err()?;

        let mut n = self.node.dir().ok_or(ErrorKind::NotADirectory)?;

        if Arc::ptr_eq(&src, &self.node) {
            if n.items.contains_key(dst_file.as_ref()) {
                return Err(ErrorKind::AlreadyExists.into());
            }
            let src = n.items.remove(src_file).ok_or(ErrorKind::NotFound)?;
            n.items.insert(dst_file.into(), src);
        } else {
            let Entry::Vacant(dst) = n.items.entry(dst_file.into()) else {
                return Err(ErrorKind::AlreadyExists.into());
            };
            let mut v = src.dir().ok_or(ErrorKind::NotADirectory)?;
            let src = v.items.remove(src_file).ok_or(ErrorKind::NotFound)?;
            v.stamp.modify();
            drop(v);
            *dst.insert(src).1.write() = Arc::downgrade(&self.node);
        }
        n.stamp.modify();

        Ok(())
    }

    pub fn unlink(&self, file: &str, is_dir: bool) -> Result<(), errors::StreamError> {
        self.access.write_or_err()?;

        let mut n = self.node.dir().ok_or(ErrorKind::NotADirectory)?;
        let v = n.items.get(file).ok_or(ErrorKind::NotFound)?;

        if is_dir {
            if !v.dir().ok_or(ErrorKind::NotADirectory)?.is_empty() {
                return Err(ErrorKind::DirectoryNotEmpty.into());
            }
        } else if v.is_dir() {
            return Err(ErrorKind::IsADirectory.into());
        }
        n.items.remove(file);

        Ok(())
    }

    pub fn read_directory(&self) -> Result<DirEntryAccessor, errors::StreamError> {
        self.access.read_or_err()?;

        if self.node.is_dir() {
            Ok(DirEntryAccessor(DirEntryInner::Current(self.node.clone())))
        } else {
            Err(ErrorKind::NotADirectory.into())
        }
    }

    pub fn read_link(&self) -> Result<String, errors::StreamError> {
        self.access.read_or_err()?;

        let mut v = self.node.link().ok_or(ErrorKind::InvalidInput)?;
        v.stamp.access();
        Ok(v.get())
    }
}

pub struct FileAccessor {
    access: AccessMode,
    file: Arc<Node>,
    cursor: usize,
    closed: bool,
}

impl FileAccessor {
    #[inline(always)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[inline(always)]
    pub fn file(&self) -> &Arc<Node> {
        &self.file
    }

    pub fn read(&mut self, len: usize) -> Result<Vec<u8>, errors::StreamError> {
        if self.closed {
            return Err(errors::StreamError::closed());
        }
        self.access.read_or_err()?;

        let mut file = self.file.try_file()?;
        let (s, l) = file.read(len, self.cursor);
        self.cursor += l;
        let mut ret = vec![0u8; l];
        ret[..s.len()].copy_from_slice(s);
        Ok(ret)
    }

    pub fn skip(&mut self, len: usize) -> Result<usize, errors::StreamError> {
        if self.closed {
            return Err(errors::StreamError::closed());
        }
        self.access.read_or_err()?;

        let mut file = self.file.try_file()?;
        let (_, l) = file.read(len, self.cursor);
        self.cursor += l;
        Ok(l)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), errors::StreamError> {
        if self.closed {
            return Err(errors::StreamError::closed());
        }
        self.access.write_or_err()?;

        self.file.try_file()?.write(buf, self.cursor)?;
        self.cursor += buf.len();
        Ok(())
    }

    #[inline(always)]
    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn poll(&self) -> AnyResult<Pollable> {
        Ok(Pollable::new())
    }
}

pub struct DirEntryAccessor(DirEntryInner);

enum DirEntryInner {
    Current(Arc<Node>),
    Parent(Arc<Node>),
    Item(Arc<str>, Arc<Node>),
    None,
}

impl Iterator for DirEntryAccessor {
    type Item = wasi::filesystem::types::DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match replace(&mut self.0, DirEntryInner::None) {
            DirEntryInner::Current(d) => {
                self.0 = DirEntryInner::Parent(d);

                Some(wasi::filesystem::types::DirectoryEntry {
                    name: ".".to_string(),
                    type_: wasi::filesystem::types::DescriptorType::Directory,
                })
            }
            DirEntryInner::Parent(d) => {
                let k = d
                    .dir()
                    .expect("Expected directory")
                    .items
                    .first_key_value()
                    .map(|(v, _)| v.clone());
                if let Some(k) = k {
                    self.0 = DirEntryInner::Item(k, d);
                }

                Some(wasi::filesystem::types::DirectoryEntry {
                    name: "..".to_string(),
                    type_: wasi::filesystem::types::DescriptorType::Directory,
                })
            }
            DirEntryInner::Item(c, d) => {
                let dir = d.dir().expect("Expected directory");
                let mut it = dir.items.range(c..);
                let (k, v) = it.next()?;
                let ret = wasi::filesystem::types::DirectoryEntry {
                    name: k.to_string(),
                    type_: v.file_type(),
                };

                let n = it.next().map(|(v, _)| v.clone());
                if let Some(k) = n {
                    drop(dir);
                    self.0 = DirEntryInner::Item(k, d);
                }

                Some(ret)
            }
            DirEntryInner::None => None,
        }
    }
}

pub type Pollable = crate::NullPollable;
