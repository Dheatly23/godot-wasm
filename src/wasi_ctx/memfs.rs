use std::any::Any;
use std::collections::btree_map::{BTreeMap, Entry};
use std::fmt::Debug;
use std::io::{Cursor, IoSlice, IoSliceMut, Read, SeekFrom, Write};
use std::mem::transmute;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use async_trait::async_trait;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use wasi_common::dir::{OpenResult, ReaddirCursor, ReaddirEntity};
use wasi_common::file::{Advice, FdFlags, FileType, Filestat, OFlags};
use wasi_common::{Error, ErrorExt, SystemTimeSpec, WasiDir, WasiFile};

use crate::wasi_ctx::timestamp::FileTimestamp;

pub const MAX_SYMLINK_DEREF: usize = 16;

static ILLEGAL_CHARS: &[char] = &['\\', '/', ':', '*', '?', '\"', '\'', '<', '>', '|'];

fn _set_times(stamp: &FileTimestamp, mtime: Option<SystemTimeSpec>, atime: Option<SystemTimeSpec>) {
    let (m, a) = match (mtime, atime) {
        (None, None) => return,
        (Some(SystemTimeSpec::Absolute(a)), None) => (Some(a.into_std()), None),
        (Some(SystemTimeSpec::SymbolicNow), None) => (Some(SystemTime::now()), None),
        (None, Some(SystemTimeSpec::Absolute(b))) => (None, Some(b.into_std())),
        (None, Some(SystemTimeSpec::SymbolicNow)) => (None, Some(SystemTime::now())),
        (Some(SystemTimeSpec::Absolute(a)), Some(SystemTimeSpec::Absolute(b))) => {
            (Some(a.into_std()), Some(b.into_std()))
        }
        (Some(SystemTimeSpec::SymbolicNow), Some(SystemTimeSpec::Absolute(b))) => {
            (Some(SystemTime::now()), Some(b.into_std()))
        }
        (Some(SystemTimeSpec::Absolute(a)), Some(SystemTimeSpec::SymbolicNow)) => {
            (Some(a.into_std()), Some(SystemTime::now()))
        }
        (Some(SystemTimeSpec::SymbolicNow), Some(SystemTimeSpec::SymbolicNow)) => {
            let n = SystemTime::now();
            (Some(n), Some(n))
        }
    };

    if let Some(m) = m {
        stamp.mtime.set_stamp(m);
    }
    if let Some(a) = a {
        stamp.atime.set_stamp(a);
    }
}

fn _touch_read(stamp: &FileTimestamp, time: Option<SystemTime>) {
    stamp.atime.set_stamp(time.unwrap_or_else(SystemTime::now));
}

fn _touch_write(stamp: &FileTimestamp, time: Option<SystemTime>) {
    let t = time.unwrap_or_else(SystemTime::now);
    stamp.mtime.set_stamp(t);
    stamp.atime.set_stamp(t);
}

pub enum FileEntry<'a> {
    Vacant(VacantFile<'a>),
    Occupied(OccupiedFile),
}

impl FileEntry<'_> {
    pub fn or_insert<F, E>(self, f: F) -> Result<Arc<dyn Node>, E>
    where
        F: FnOnce(Weak<dyn Node>, FileTimestamp) -> Result<Arc<dyn Node>, E>,
    {
        match self {
            Self::Vacant(v) => v.insert(f),
            Self::Occupied(v) => Ok(v.into_inner()),
        }
    }
}

pub struct VacantFile<'a> {
    guard: DirContentGuard<'a>,
    folder: Arc<dyn Node>,
    name: &'a str,
    time: SystemTime,
}

impl VacantFile<'_> {
    pub fn insert<F, E>(mut self, f: F) -> Result<Arc<dyn Node>, E>
    where
        F: FnOnce(Weak<dyn Node>, FileTimestamp) -> Result<Arc<dyn Node>, E>,
    {
        let file = f(
            Arc::downgrade(&self.folder),
            FileTimestamp::with_time(self.time),
        )?;
        self.guard.insert(self.name.to_owned(), file.clone());
        _touch_write(self.folder.timestamp(), Some(self.time));
        drop(self);
        Ok(file)
    }
}

pub struct OccupiedFile {
    file: Arc<dyn Node>,
}

impl Deref for OccupiedFile {
    type Target = Arc<dyn Node>;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl OccupiedFile {
    pub fn into_inner(self) -> Arc<dyn Node> {
        self.file
    }
}

pub fn open<'a>(
    path: &'a str,
    mut node: Arc<dyn Node>,
    root: &Option<Arc<Dir>>,
    follow_symlink: bool,
    create_intermediate_dir: bool,
) -> Result<FileEntry<'a>, Error> {
    let time = SystemTime::now();

    let mut it = Utf8Path::new(path).components().peekable();
    while let Some(c) = it.next() {
        node = match c {
            Utf8Component::Prefix(_) | Utf8Component::RootDir => {
                return Err(Error::invalid_argument())
            }
            Utf8Component::CurDir => continue,
            Utf8Component::ParentDir => node.parent().unwrap_or(node),
            Utf8Component::Normal(p) => {
                if p.contains(ILLEGAL_CHARS) {
                    return Err(Error::invalid_argument());
                }

                let n = if it.peek().is_none() {
                    let Some(n) = node.as_any().downcast_ref::<Dir>() else {
                        return Err(Error::not_dir());
                    };
                    let content = n.content.write();
                    if let Some(v) = content.get(p) {
                        _touch_read(n.timestamp(), Some(time));
                        v.clone()
                    } else {
                        // SAFETY: Node is living at least as long as guard is.
                        let guard = unsafe { transmute(content) };
                        return Ok(FileEntry::Vacant(VacantFile {
                            guard,
                            folder: node,
                            name: p,
                            time,
                        }));
                    }
                } else if create_intermediate_dir {
                    let Some(n) = node.as_any().downcast_ref::<Dir>() else {
                        return Err(Error::not_dir());
                    };
                    let mut content = n.content.write();
                    match content.entry(p.to_owned()) {
                        Entry::Vacant(v) => {
                            _touch_write(n.timestamp(), Some(time));
                            v.insert(Arc::new(Dir::new(Arc::downgrade(&node)))).clone()
                        }
                        Entry::Occupied(v) => {
                            _touch_read(n.timestamp(), Some(time));
                            v.get().clone()
                        }
                    }
                } else if let Some(n) = node.child(p) {
                    _touch_read(node.timestamp(), Some(time));
                    n
                } else {
                    return Err(Error::not_found());
                };

                if follow_symlink {
                    match n.link_deref(root, MAX_SYMLINK_DEREF) {
                        Some(v) => v,
                        None => return Err(Error::not_found()),
                    }
                } else {
                    n
                }
            }
        };
    }

    Ok(FileEntry::Occupied(OccupiedFile { file: node }))
}

#[derive(Clone, Copy, Debug)]
pub struct Capability {
    pub read: bool,
    pub write: bool,
}

#[derive(Clone, Debug)]
struct CapAccessor<T: ?Sized> {
    capability: Capability,
    root: Option<Arc<Dir>>,
    value: T,
}

impl<T: Sized> CapAccessor<T> {
    #[inline(always)]
    fn new(root: Option<Arc<Dir>>, value: T) -> Self {
        Self {
            capability: Capability {
                read: true,
                write: true,
            },
            root,
            value,
        }
    }

    #[inline(always)]
    fn set_cap(mut self, cap: Capability) -> Self {
        self.capability = cap;
        self
    }
}

impl<T: ?Sized> Deref for CapAccessor<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

pub trait Node: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn parent(&self) -> Option<Arc<dyn Node>>;
    fn set_parent(&self, new_parent: Weak<dyn Node>);
    fn filetype(&self) -> FileType;
    fn filestat(&self) -> Filestat;
    fn timestamp(&self) -> &FileTimestamp;
    fn child(&self, _name: &str) -> Option<Arc<dyn Node>> {
        None
    }
    fn open(
        self: Arc<Self>,
        _root: Option<Arc<Dir>>,
        _cap: Capability,
        _follow_symlink: bool,
        _oflags: OFlags,
        _fdflags: FdFlags,
    ) -> Result<OpenResult, Error> {
        Err(Error::not_supported())
    }
    fn as_link(&self) -> Option<Utf8PathBuf> {
        None
    }
    fn link_deref(self: Arc<Self>, _root: &Option<Arc<Dir>>, _n: usize) -> Option<Arc<dyn Node>>;
}

type DirContent = RwLock<BTreeMap<String, Arc<dyn Node>>>;
type DirContentGuard<'a> = RwLockWriteGuard<'a, BTreeMap<String, Arc<dyn Node>>>;

pub struct Dir {
    parent: RwLock<Weak<dyn Node>>,
    stamp: FileTimestamp,

    pub content: DirContent,
}

impl Debug for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dir {{ ... }}")
    }
}

impl Dir {
    #[inline]
    pub fn new(parent: Weak<dyn Node>) -> Self {
        Self::with_timestamp(parent, FileTimestamp::new())
    }

    #[inline]
    pub fn with_timestamp(parent: Weak<dyn Node>, stamp: FileTimestamp) -> Self {
        Self {
            parent: RwLock::new(parent),
            stamp,

            content: RwLock::default(),
        }
    }
}

impl Node for Dir {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn parent(&self) -> Option<Arc<dyn Node>> {
        self.parent.read().upgrade()
    }

    fn set_parent(&self, new_parent: Weak<dyn Node>) {
        *self.parent.write() = new_parent;
    }

    fn filetype(&self) -> FileType {
        FileType::Directory
    }

    fn filestat(&self) -> Filestat {
        Filestat {
            device_id: 0,
            inode: 0,
            filetype: self.filetype(),
            nlink: 0,
            size: 0,
            atim: self.stamp.atime.get_stamp(),
            mtim: self.stamp.mtime.get_stamp(),
            ctim: Some(self.stamp.ctime),
        }
    }

    fn timestamp(&self) -> &FileTimestamp {
        &self.stamp
    }

    fn child(&self, name: &str) -> Option<Arc<dyn Node>> {
        self.content.read().get(name).cloned()
    }

    fn open(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        _follow_symlink: bool,
        oflags: OFlags,
        fdflags: FdFlags,
    ) -> Result<OpenResult, Error> {
        if oflags.contains(OFlags::TRUNCATE) || !fdflags.is_empty() {
            return Err(Error::not_supported());
        }

        Ok(OpenResult::Dir(Box::new(
            CapAccessor::new(root, self).set_cap(cap),
        )))
    }

    fn link_deref(self: Arc<Self>, _root: &Option<Arc<Dir>>, _n: usize) -> Option<Arc<dyn Node>> {
        Some(self as _)
    }
}

#[async_trait]
impl WasiDir for CapAccessor<Arc<Dir>> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn open_file(
        &self,
        symlink_follow: bool,
        path: &str,
        oflags: OFlags,
        read: bool,
        write: bool,
        fdflags: FdFlags,
    ) -> Result<OpenResult, Error> {
        if fdflags.intersects(FdFlags::DSYNC | FdFlags::RSYNC | FdFlags::SYNC | FdFlags::NONBLOCK) {
            return Err(Error::not_supported());
        }

        if !self.capability.write && (write || oflags.contains(OFlags::CREATE)) {
            return Err(Error::perm());
        }
        if !self.capability.read && (read || !oflags.contains(OFlags::CREATE)) {
            return Err(Error::perm());
        }

        open(
            path,
            self.value.clone(),
            &self.root,
            oflags.contains(OFlags::CREATE),
            false,
        )?
        .or_insert(|parent, stamp| -> Result<_, Error> {
            if oflags.contains(OFlags::DIRECTORY) {
                Ok(Arc::new(Dir::with_timestamp(parent, stamp)))
            } else {
                Ok(Arc::new(File::with_timestamp(parent, stamp)))
            }
        })?
        .open(
            self.root.clone(),
            Capability { read, write },
            symlink_follow,
            oflags,
            fdflags,
        )
    }

    async fn create_dir(&self, path: &str) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        if matches!(path, "" | "." | "..") || path.contains(ILLEGAL_CHARS) {
            return Err(Error::invalid_argument());
        }

        match self.content.write().entry(path.to_owned()) {
            Entry::Occupied(_) => Err(Error::exist()),
            Entry::Vacant(v) => {
                v.insert(Arc::new(Dir::new(Arc::downgrade(&self.value) as _)));

                _touch_write(self.timestamp(), None);
                Ok(())
            }
        }
    }

    async fn readdir(
        &self,
        cursor: ReaddirCursor,
    ) -> Result<Box<dyn Iterator<Item = Result<ReaddirEntity, Error>> + Send>, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }

        let mut ix: usize = u64::from(cursor)
            .try_into()
            .map_err(|_| Error::invalid_argument())?;

        let content = self.content.read();
        let mut ret = Vec::with_capacity((content.len() + 2).saturating_sub(ix));
        if ix == 0 {
            ret.push(Ok(ReaddirEntity {
                next: <_>::from(1),
                name: ".".to_owned(),
                inode: 0,
                filetype: self.filetype(),
            }));
            ix = 1;
        }
        if ix == 1 {
            ret.push(Ok(ReaddirEntity {
                next: <_>::from(2),
                name: "..".to_owned(),
                inode: 0,
                filetype: match self.parent() {
                    Some(v) => v.filetype(),
                    None => self.filetype(),
                },
            }));
            ix = 2;
        }
        for (k, v) in content.iter().skip(ix - 2) {
            ix += 1;
            ret.push(Ok(ReaddirEntity {
                next: <_>::from(ix as u64),
                name: k.to_owned(),
                inode: 0,
                filetype: v.filetype(),
            }));
        }

        _touch_read(self.timestamp(), None);
        Ok(Box::new(ret.into_iter()))
    }

    async fn symlink(&self, old_path: &str, new_path: &str) -> Result<(), Error> {
        if !self.capability.read || !self.capability.write {
            return Err(Error::perm());
        }

        if matches!(new_path, "" | "." | "..") || new_path.contains(ILLEGAL_CHARS) {
            return Err(Error::invalid_argument());
        }

        let mut content = self.content.write();
        match content.entry(new_path.to_owned()) {
            Entry::Occupied(_) => Err(Error::exist()),
            Entry::Vacant(v) => {
                v.insert(Arc::new(Link::new(
                    Arc::downgrade(&self.value) as _,
                    Utf8PathBuf::from(old_path),
                )));

                _touch_write(self.timestamp(), None);
                Ok(())
            }
        }
    }

    async fn remove_dir(&self, path: &str) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        let mut content = self.content.write();
        match content.get(path) {
            None => Err(Error::not_found()),
            Some(v) if v.filetype() == FileType::Directory => {
                content.remove(path);

                _touch_write(self.timestamp(), None);
                Ok(())
            }
            _ => Err(Error::not_dir()),
        }
    }

    async fn unlink_file(&self, path: &str) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        let mut content = self.content.write();
        match content.get(path) {
            None => Err(Error::not_found()),
            Some(v) if v.filetype() != FileType::Directory => {
                content.remove(path);

                _touch_write(self.timestamp(), None);
                Ok(())
            }
            _ => Err(Error::not_dir()),
        }
    }

    async fn read_link(&self, path: &str) -> Result<PathBuf, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }

        match self.child(path) {
            None => Err(Error::not_found()),
            Some(v) => match v.as_link() {
                None => Err(Error::not_supported()),
                Some(v) => Ok(v.into()),
            },
        }
    }

    async fn get_filestat(&self) -> Result<Filestat, Error> {
        Ok(self.filestat())
    }

    async fn get_path_filestat(
        &self,
        path: &str,
        follow_symlinks: bool,
    ) -> Result<Filestat, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }

        match path {
            "" => return Err(Error::invalid_argument()),
            "." => return Ok(self.filestat()),
            ".." => match self.parent() {
                Some(v) => Ok(v.filestat()),
                None => Ok(self.filestat()),
            },
            path => {
                let mut node = self.child(path);
                if follow_symlinks {
                    node = node.and_then(|n| n.link_deref(&self.root, MAX_SYMLINK_DEREF));
                }
                match node {
                    Some(node) => Ok(node.filestat()),
                    None => Err(Error::not_found()),
                }
            }
        }
    }

    async fn rename(
        &self,
        path: &str,
        dest_dir: &dyn WasiDir,
        dest_path: &str,
    ) -> Result<(), Error> {
        if !self.capability.read || !self.capability.write {
            return Err(Error::perm());
        }

        if matches!(dest_path, "" | "." | "..") || dest_path.contains(ILLEGAL_CHARS) {
            return Err(Error::invalid_argument());
        }

        let Some(dest) = dest_dir.as_any().downcast_ref::<Self>() else {
            return Err(Error::not_supported());
        };

        if !dest.capability.write {
            return Err(Error::perm());
        }

        let mut content = self.content.write();
        let node = content.get(path).ok_or_else(Error::not_found)?;
        let time = SystemTime::now();

        if Arc::ptr_eq(self, dest) {
            if dest_path == path {
                return Ok(());
            }

            let node = node.clone();
            match content.entry(dest_path.to_owned()) {
                Entry::Occupied(_) => return Err(Error::exist()),
                Entry::Vacant(v) => v.insert(node),
            };
        } else {
            let mut p: Arc<dyn Node> = dest.value.clone();
            while let Some(p_) = p.parent() {
                if Arc::ptr_eq(node, &p_) {
                    return Err(Error::not_supported());
                }
                p = p_;
            }

            match dest.content.write().entry(dest_path.to_owned()) {
                Entry::Occupied(_) => return Err(Error::exist()),
                Entry::Vacant(v) => v.insert(node.clone()),
            };

            node.set_parent(Arc::downgrade(dest) as _);
            _touch_write(dest.timestamp(), Some(time));
        }

        content.remove(path);
        _touch_write(self.timestamp(), Some(time));
        Ok(())
    }

    async fn set_times(
        &self,
        path: &str,
        atime: Option<SystemTimeSpec>,
        mtime: Option<SystemTimeSpec>,
        follow_symlinks: bool,
    ) -> Result<(), Error> {
        match path {
            "" => return Err(Error::invalid_argument()),
            "." => _set_times(self.timestamp(), mtime, atime),
            ".." => _set_times(
                match &self.parent() {
                    Some(v) => v.timestamp(),
                    None => self.timestamp(),
                },
                mtime,
                atime,
            ),
            path => {
                let mut node = self.child(path);
                if follow_symlinks {
                    node = node.and_then(|n| n.link_deref(&self.root, MAX_SYMLINK_DEREF));
                }
                match node {
                    Some(node) => _set_times(node.timestamp(), mtime, atime),
                    None => return Err(Error::not_found()),
                }
            }
        }

        Ok(())
    }
}

pub struct File {
    parent: RwLock<Weak<dyn Node>>,
    stamp: FileTimestamp,

    pub content: RwLock<Vec<u8>>,
}

impl File {
    #[allow(dead_code)]
    #[inline]
    pub fn new(parent: Weak<dyn Node>) -> Self {
        Self::with_timestamp(parent, FileTimestamp::new())
    }

    #[inline]
    pub fn with_timestamp(parent: Weak<dyn Node>, stamp: FileTimestamp) -> Self {
        Self {
            parent: RwLock::new(parent),
            stamp,

            content: RwLock::default(),
        }
    }
}

impl Node for File {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn parent(&self) -> Option<Arc<dyn Node>> {
        self.parent.read().upgrade()
    }

    fn set_parent(&self, new_parent: Weak<dyn Node>) {
        *self.parent.write() = new_parent;
    }

    fn filetype(&self) -> FileType {
        FileType::RegularFile
    }

    fn filestat(&self) -> Filestat {
        Filestat {
            device_id: 0,
            inode: 0,
            filetype: self.filetype(),
            nlink: 0,
            size: self.content.read().len() as _,
            atim: self.stamp.atime.get_stamp(),
            mtim: self.stamp.mtime.get_stamp(),
            ctim: Some(self.stamp.ctime),
        }
    }

    fn timestamp(&self) -> &FileTimestamp {
        &self.stamp
    }

    fn open(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        _follow_symlink: bool,
        oflags: OFlags,
        fdflags: FdFlags,
    ) -> Result<OpenResult, Error> {
        if oflags.contains(OFlags::DIRECTORY) {
            return Err(Error::not_dir());
        }

        if oflags.contains(OFlags::TRUNCATE) {
            if !cap.write {
                return Err(Error::perm());
            }

            self.content.write().clear()
        }

        let ptr = if fdflags.contains(FdFlags::APPEND) {
            self.content.read().len() as u64
        } else {
            0
        };

        Ok(OpenResult::File(Box::new(
            CapAccessor::new(
                root,
                OpenFile {
                    fdflags,
                    file: self,
                    ptr: Mutex::new(ptr),
                },
            )
            .set_cap(cap),
        )))
    }

    fn link_deref(self: Arc<Self>, _root: &Option<Arc<Dir>>, _n: usize) -> Option<Arc<dyn Node>> {
        Some(self as _)
    }
}

struct OpenFile {
    fdflags: FdFlags,
    file: Arc<File>,
    ptr: Mutex<u64>,
}

#[async_trait]
impl WasiFile for CapAccessor<OpenFile> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(self.file.filetype())
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Ok(self.fdflags)
    }

    async fn set_fdflags(&mut self, _flags: FdFlags) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn get_filestat(&self) -> Result<Filestat, Error> {
        Ok(self.file.filestat())
    }

    async fn set_filestat_size(&self, size: u64) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        self.file
            .content
            .write()
            .truncate(usize::try_from(size).unwrap_or(usize::MAX));

        _touch_write(self.file.timestamp(), None);
        Ok(())
    }

    async fn advise(&self, _offset: u64, _len: u64, _advice: Advice) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn set_times(
        &self,
        atime: Option<SystemTimeSpec>,
        mtime: Option<SystemTimeSpec>,
    ) -> Result<(), Error> {
        _set_times(self.file.timestamp(), mtime, atime);
        Ok(())
    }

    async fn read_vectored<'a>(&self, bufs: &mut [IoSliceMut<'a>]) -> Result<u64, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }
        if self.fdflags.contains(FdFlags::APPEND) {
            return Ok(0);
        }

        let mut ptr = self.ptr.lock();
        let content = self.file.content.read();

        let mut cursor = Cursor::new(&*content);
        cursor.set_position(*ptr);
        let n = cursor.read_vectored(bufs)?;
        *ptr = cursor.position();

        _touch_read(self.file.timestamp(), None);
        Ok(n as _)
    }

    async fn read_vectored_at<'a>(
        &self,
        bufs: &mut [IoSliceMut<'a>],
        offset: u64,
    ) -> Result<u64, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }
        if self.fdflags.contains(FdFlags::APPEND) {
            return Ok(0);
        }

        let ptr = self.ptr.lock();
        let ix: usize = ptr
            .checked_add(offset)
            .ok_or_else(Error::overflow)?
            .try_into()
            .unwrap_or(usize::MAX);
        let content = self.file.content.read();

        if ix >= content.len() {
            return Ok(0);
        }
        let n = (&content[ix..]).read_vectored(bufs)?;

        _touch_read(self.file.timestamp(), None);
        Ok(n as _)
    }

    async fn write_vectored<'a>(&self, bufs: &[IoSlice<'a>]) -> Result<u64, Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        let mut ptr = self.ptr.lock();
        let mut content = self.file.content.write();
        let n;

        if self.fdflags.contains(FdFlags::APPEND) {
            n = content.write_vectored(bufs)?;
            *ptr = content.len() as _;
        } else {
            let mut cursor = Cursor::new(&mut *content);
            cursor.set_position(*ptr);
            n = cursor.write_vectored(bufs)?;
            *ptr = cursor.position();
        }

        _touch_write(self.file.timestamp(), None);
        Ok(n as _)
    }

    async fn write_vectored_at<'a>(&self, bufs: &[IoSlice<'a>], offset: u64) -> Result<u64, Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }
        if self.fdflags.contains(FdFlags::APPEND) {
            return Err(Error::not_supported());
        }

        let ptr = self.ptr.lock();
        let Ok(ix): Result<usize, _> = ptr
            .checked_add(offset)
            .ok_or_else(Error::overflow)?
            .try_into()
        else {
            return Ok(0);
        };
        let mut content = self.file.content.write();

        if ix >= content.len() {
            content.resize(ix, 0);
        }
        let mut cursor = Cursor::new(&mut *content);
        cursor.set_position(ix as _);
        let n = cursor.write_vectored(bufs)?;

        _touch_write(self.file.timestamp(), None);
        Ok(n as _)
    }

    async fn seek(&self, pos: SeekFrom) -> Result<u64, Error> {
        if self.fdflags.contains(FdFlags::APPEND) && pos != SeekFrom::End(0) {
            return Err(Error::not_supported());
        }

        let mut ptr = self.ptr.lock();
        let content = self.file.content.read();

        *ptr = match pos {
            SeekFrom::Start(v) => v,
            SeekFrom::Current(v) => ptr.checked_add_signed(v).ok_or_else(Error::overflow)?,
            SeekFrom::End(v) => (content.len() as u64)
                .checked_add_signed(v)
                .ok_or_else(Error::overflow)?,
        }
        .min(content.len() as _);

        Ok(*ptr)
    }

    async fn peek(&self, buf: &mut [u8]) -> Result<u64, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }
        if self.fdflags.contains(FdFlags::APPEND) {
            return Ok(0);
        }

        let ptr = self.ptr.lock();
        let content = self.file.content.read();

        let n = content
            .get(usize::try_from(*ptr).unwrap_or(usize::MAX)..)
            .unwrap_or_default()
            .read(buf)?;

        _touch_read(self.file.timestamp(), None);
        Ok(n as _)
    }

    fn num_ready_bytes(&self) -> Result<u64, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }
        if self.fdflags.contains(FdFlags::APPEND) {
            return Ok(0);
        }

        let ptr = self.ptr.lock();
        let content = self.file.content.read();

        if let Ok(ix) = usize::try_from(*ptr) {
            Ok(content.len().saturating_sub(ix) as _)
        } else {
            Ok(0)
        }
    }

    async fn readable(&self) -> Result<(), Error> {
        if !self.capability.read {
            Err(Error::perm())
        } else {
            Ok(())
        }
    }

    async fn writable(&self) -> Result<(), Error> {
        if !self.capability.write {
            Err(Error::perm())
        } else {
            Ok(())
        }
    }
}

pub struct Link {
    parent: RwLock<Weak<dyn Node>>,
    stamp: FileTimestamp,

    pub path: Utf8PathBuf,
}

impl Link {
    pub fn new(parent: Weak<dyn Node>, path: Utf8PathBuf) -> Self {
        Self {
            parent: RwLock::new(parent),
            stamp: FileTimestamp::new(),

            path,
        }
    }
}

impl Node for Link {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn parent(&self) -> Option<Arc<dyn Node>> {
        self.parent.read().upgrade()
    }

    fn set_parent(&self, new_parent: Weak<dyn Node>) {
        *self.parent.write() = new_parent;
    }

    fn filetype(&self) -> FileType {
        FileType::SymbolicLink
    }

    fn filestat(&self) -> Filestat {
        Filestat {
            device_id: 0,
            inode: 0,
            filetype: self.filetype(),
            nlink: 0,
            size: self.path.as_os_str().len() as _,
            atim: self.stamp.atime.get_stamp(),
            mtim: self.stamp.mtime.get_stamp(),
            ctim: Some(self.stamp.ctime),
        }
    }

    fn timestamp(&self) -> &FileTimestamp {
        &self.stamp
    }

    fn open(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        follow_symlink: bool,
        oflags: OFlags,
        fdflags: FdFlags,
    ) -> Result<OpenResult, Error> {
        if !follow_symlink {
            return if oflags.intersects(OFlags::DIRECTORY | OFlags::TRUNCATE) || !fdflags.is_empty()
            {
                Err(Error::invalid_argument())
            } else {
                Ok(OpenResult::File(Box::new(
                    CapAccessor::new(root, self).set_cap(cap),
                )))
            };
        }

        self.link_deref(&root, MAX_SYMLINK_DEREF)
            .ok_or_else(Error::not_found)
            .and_then(|n| n.open(root, cap, follow_symlink, oflags, fdflags))
    }

    fn as_link(&self) -> Option<Utf8PathBuf> {
        Some(self.path.clone())
    }

    fn link_deref(self: Arc<Self>, root: &Option<Arc<Dir>>, mut n: usize) -> Option<Arc<dyn Node>> {
        n = match n.checked_sub(1) {
            Some(v) => v,
            None => return None,
        };

        let mut node = self.parent();
        for c in self.path.components() {
            node = match c {
                Utf8Component::RootDir => root.clone().map(|v| v as _),
                Utf8Component::CurDir => continue,
                Utf8Component::ParentDir => node.map(|n| n.parent().unwrap_or(n)),
                Utf8Component::Normal(name) => node
                    .and_then(|n| n.child(name))
                    .and_then(|v| v.link_deref(root, n)),
                Utf8Component::Prefix(_) => return None,
            };
        }

        node
    }
}

#[async_trait]
impl WasiFile for CapAccessor<Arc<Link>> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn get_filetype(&self) -> Result<FileType, Error> {
        Ok(self.filetype())
    }

    async fn get_fdflags(&self) -> Result<FdFlags, Error> {
        Err(Error::not_supported())
    }

    async fn set_fdflags(&mut self, _flags: FdFlags) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn get_filestat(&self) -> Result<Filestat, Error> {
        Ok(self.filestat())
    }

    async fn set_filestat_size(&self, _size: u64) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn advise(&self, _offset: u64, _len: u64, _advice: Advice) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn set_times(
        &self,
        atime: Option<SystemTimeSpec>,
        mtime: Option<SystemTimeSpec>,
    ) -> Result<(), Error> {
        _set_times(self.timestamp(), mtime, atime);
        Ok(())
    }

    async fn read_vectored<'a>(&self, _bufs: &mut [IoSliceMut<'a>]) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    async fn read_vectored_at<'a>(
        &self,
        _bufs: &mut [IoSliceMut<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    async fn write_vectored<'a>(&self, _bufs: &[IoSlice<'a>]) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    async fn write_vectored_at<'a>(
        &self,
        _bufs: &[IoSlice<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    async fn seek(&self, _pos: SeekFrom) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    async fn peek(&self, _buf: &mut [u8]) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    fn num_ready_bytes(&self) -> Result<u64, Error> {
        Err(Error::not_supported())
    }

    async fn readable(&self) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn writable(&self) -> Result<(), Error> {
        Err(Error::not_supported())
    }
}
