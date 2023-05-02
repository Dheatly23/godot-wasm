use std::any::Any;
use std::collections::btree_map::{BTreeMap, Entry};
use std::fmt::Debug;
use std::io::{Cursor, IoSlice, IoSliceMut, Read, SeekFrom, Write};
use std::ops::Deref;
use std::path::{Component, PathBuf};
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use wasi_common::dir::{ReaddirCursor, ReaddirEntity};
use wasi_common::file::{Advice, FdFlags, FileType, Filestat, OFlags};
use wasi_common::{Error, ErrorExt, SystemTimeSpec, WasiDir, WasiFile};

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
    fn filetype(&self) -> FileType;
    fn child(&self, _name: &str) -> Option<Arc<dyn Node>> {
        None
    }
    fn as_dir(
        self: Arc<Self>,
        _root: Option<Arc<Dir>>,
        _cap: Capability,
        _follow_symlink: bool,
    ) -> Result<Box<dyn WasiDir>, Error> {
        Err(Error::not_supported())
    }
    fn as_file(
        self: Arc<Self>,
        _root: Option<Arc<Dir>>,
        _cap: Capability,
        _follow_symlink: bool,
        _oflags: OFlags,
        _fdflags: FdFlags,
    ) -> Result<Box<dyn WasiFile>, Error> {
        Err(Error::not_supported())
    }
    fn as_link(&self) -> Option<PathBuf> {
        None
    }
    fn link_deref(self: Arc<Self>, _root: &Option<Arc<Dir>>) -> Option<Arc<dyn Node>>;
}

pub struct Dir {
    parent: Weak<dyn Node>,

    pub content: RwLock<BTreeMap<String, Arc<dyn Node>>>,
}

impl Debug for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dir {{ ... }}")
    }
}

impl Dir {
    pub fn new(parent: Weak<dyn Node>) -> Self {
        Self {
            parent,

            content: RwLock::default(),
        }
    }
}

impl Node for Dir {
    fn as_any(&self) -> &dyn Any {
        &*self
    }

    fn parent(&self) -> Option<Arc<dyn Node>> {
        self.parent.upgrade()
    }

    fn filetype(&self) -> FileType {
        FileType::Directory
    }

    fn child(&self, name: &str) -> Option<Arc<dyn Node>> {
        self.content.read().get(name).cloned()
    }

    fn as_dir(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        _follow_symlink: bool,
    ) -> Result<Box<dyn WasiDir>, Error> {
        Ok(Box::new(CapAccessor::new(root, self).set_cap(cap)))
    }

    fn as_file(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        _follow_symlink: bool,
        oflags: OFlags,
        fdflags: FdFlags,
    ) -> Result<Box<dyn WasiFile>, Error> {
        if !oflags.contains(OFlags::DIRECTORY)
            || oflags.contains(OFlags::TRUNCATE)
            || !fdflags.is_empty()
        {
            return Err(Error::not_supported());
        }

        Ok(Box::new(CapAccessor::new(root, self).set_cap(cap)))
    }

    fn link_deref(self: Arc<Self>, _root: &Option<Arc<Dir>>) -> Option<Arc<dyn Node>> {
        Some(self as _)
    }
}

#[async_trait]
impl WasiDir for CapAccessor<Arc<Dir>> {
    fn as_any(&self) -> &dyn Any {
        &*self
    }

    async fn open_file(
        &self,
        symlink_follow: bool,
        path: &str,
        oflags: OFlags,
        read: bool,
        write: bool,
        fdflags: FdFlags,
    ) -> Result<Box<dyn WasiFile>, Error> {
        if fdflags.intersects(FdFlags::DSYNC | FdFlags::RSYNC | FdFlags::SYNC | FdFlags::NONBLOCK) {
            return Err(Error::not_supported());
        }
        if oflags.contains(OFlags::CREATE | OFlags::DIRECTORY) {
            return Err(Error::not_supported());
        }

        if !self.capability.write && (write || oflags.contains(OFlags::CREATE)) {
            return Err(Error::perm());
        }
        if !self.capability.read && !oflags.contains(OFlags::CREATE) {
            return Err(Error::perm());
        }

        if let Some((first, rest)) = path.split_once('/') {
            return self
                .open_dir(symlink_follow, first)
                .await?
                .open_file(symlink_follow, rest, oflags, read, write, fdflags)
                .await;
        }

        if matches!(path, "" | "." | "..") {
            return Err(Error::invalid_argument());
        }

        let file = if oflags.contains(OFlags::CREATE) {
            let mut content = self.content.write();
            loop {
                if let Some(v) = content.get(path) {
                    break v.clone();
                } else if oflags.contains(OFlags::EXCLUSIVE) {
                    return Err(Error::exist());
                } else {
                    content.insert(
                        path.to_owned(),
                        Arc::new(File::new(Arc::downgrade(&self.value) as _)),
                    );
                }
            }
        } else if let Some(v) = self.child(path) {
            v
        } else {
            return Err(Error::not_found());
        };

        file.as_file(
            self.root.clone(),
            Capability { read, write },
            symlink_follow,
            oflags,
            fdflags,
        )
    }

    async fn open_dir(&self, symlink_follow: bool, path: &str) -> Result<Box<dyn WasiDir>, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }

        let file = match path {
            "" => return Err(Error::invalid_argument()),
            "." => self.value.clone(),
            ".." => self.parent().unwrap_or_else(|| self.value.clone()),
            _ => match self.child(path) {
                Some(v) => v,
                None => return Err(Error::not_found()),
            },
        };

        file.as_dir(self.root.clone(), self.capability, symlink_follow)
    }

    async fn create_dir(&self, path: &str) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        if matches!(path, "" | "." | "..") || path.contains('/') {
            return Err(Error::invalid_argument());
        }

        match self.content.write().entry(path.to_owned()) {
            Entry::Occupied(_) => Err(Error::exist()),
            Entry::Vacant(v) => {
                v.insert(Arc::new(Dir::new(Arc::downgrade(&self.value) as _)));
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
                filetype: self.filetype(),
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

        Ok(Box::new(ret.into_iter()))
    }

    async fn symlink(&self, old_path: &str, new_path: &str) -> Result<(), Error> {
        if !self.capability.read || !self.capability.write {
            return Err(Error::perm());
        }

        if matches!(new_path, "" | "." | "..") || new_path.contains('/') {
            return Err(Error::invalid_argument());
        }

        let mut content = self.content.write();
        match content.entry(new_path.to_owned()) {
            Entry::Occupied(_) => Err(Error::exist()),
            Entry::Vacant(v) => {
                v.insert(Arc::new(Link::new(
                    Arc::downgrade(&self.value) as _,
                    PathBuf::from(old_path),
                )));
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
            Some(v) => v.as_link().ok_or_else(Error::not_supported),
        }
    }

    async fn get_filestat(&self) -> Result<Filestat, Error> {
        Ok(Filestat {
            device_id: 0,
            inode: 0,
            filetype: self.filetype(),
            nlink: 0,
            size: 0,
            atim: None,
            mtim: None,
            ctim: None,
        })
    }

    async fn get_path_filestat(
        &self,
        path: &str,
        follow_symlinks: bool,
    ) -> Result<Filestat, Error> {
        if !self.capability.read {
            return Err(Error::perm());
        }

        let node = match path {
            "" => return Err(Error::invalid_argument()),
            "." => {
                return Ok(Filestat {
                    device_id: 0,
                    inode: 0,
                    filetype: self.filetype(),
                    nlink: 0,
                    size: 0,
                    atim: None,
                    mtim: None,
                    ctim: None,
                })
            }
            ".." => self.parent(),
            path => self.child(path).and_then(|n| {
                if follow_symlinks {
                    n.link_deref(&self.root)
                } else {
                    Some(n)
                }
            }),
        };
        if let Some(node) = node {
            Ok(Filestat {
                device_id: 0,
                inode: 0,
                filetype: node.filetype(),
                nlink: 0,
                size: 0,
                atim: None,
                mtim: None,
                ctim: None,
            })
        } else {
            Err(Error::not_found())
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

        if matches!(dest_path, "" | "." | "..") || dest_path.contains('/') {
            return Err(Error::invalid_argument());
        }

        let Some(dest) = dest_dir.as_any().downcast_ref::<Self>() else { return Err(Error::not_supported()) };

        if !dest.capability.write {
            return Err(Error::perm());
        }

        let mut content = self.content.write();
        let node = content.get(path).ok_or_else(Error::not_found)?;

        if Arc::ptr_eq(&*self, &*dest) {
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
        }

        content.remove(path);
        Ok(())
    }
}

#[async_trait]
impl WasiFile for CapAccessor<Arc<Dir>> {
    fn as_any(&self) -> &dyn Any {
        &*self
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
        Ok(Filestat {
            device_id: 0,
            inode: 0,
            filetype: self.filetype(),
            nlink: 0,
            size: 0,
            atim: None,
            mtim: None,
            ctim: None,
        })
    }

    async fn set_filestat_size(&self, _size: u64) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn advise(&self, _offset: u64, _len: u64, _advice: Advice) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn allocate(&self, _offset: u64, _len: u64) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn set_times(
        &self,
        _atime: Option<SystemTimeSpec>,
        _mtime: Option<SystemTimeSpec>,
    ) -> Result<(), Error> {
        Err(Error::not_supported())
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

pub struct File {
    parent: Weak<dyn Node>,

    pub content: RwLock<Vec<u8>>,
}

impl File {
    pub fn new(parent: Weak<dyn Node>) -> Self {
        Self {
            parent,

            content: RwLock::default(),
        }
    }
}

impl Node for File {
    fn as_any(&self) -> &dyn Any {
        &*self
    }

    fn parent(&self) -> Option<Arc<dyn Node>> {
        self.parent.upgrade()
    }

    fn filetype(&self) -> FileType {
        FileType::RegularFile
    }

    fn as_dir(
        self: Arc<Self>,
        _root: Option<Arc<Dir>>,
        _cap: Capability,
        _follow_symlink: bool,
    ) -> Result<Box<dyn WasiDir>, Error> {
        Err(Error::not_dir())
    }

    fn as_file(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        _follow_symlink: bool,
        oflags: OFlags,
        fdflags: FdFlags,
    ) -> Result<Box<dyn WasiFile>, Error> {
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

        Ok(Box::new(
            CapAccessor::new(
                root,
                OpenFile {
                    fdflags,
                    file: self,
                    ptr: Mutex::new(ptr),
                },
            )
            .set_cap(cap),
        ))
    }

    fn link_deref(self: Arc<Self>, _root: &Option<Arc<Dir>>) -> Option<Arc<dyn Node>> {
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
        &*self
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
        Ok(Filestat {
            device_id: 0,
            inode: 0,
            filetype: self.file.filetype(),
            nlink: 0,
            size: self.file.content.read().len(),
            atim: None,
            mtim: None,
            ctim: None,
        })
    }

    async fn set_filestat_size(&self, size: u64) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        self.file
            .content
            .write()
            .truncate(usize::try_from(size).unwrap_or(usize::MAX));
        Ok(())
    }

    async fn advise(&self, _offset: u64, _len: u64, _advice: Advice) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn allocate(&self, _offset: u64, _len: u64) -> Result<(), Error> {
        if !self.capability.write {
            return Err(Error::perm());
        }

        Ok(())
    }

    async fn set_times(
        &self,
        _atime: Option<SystemTimeSpec>,
        _mtime: Option<SystemTimeSpec>,
    ) -> Result<(), Error> {
        Err(Error::not_supported())
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
            .try_into() else { return Ok(0) };
        let mut content = self.file.content.write();

        if ix >= content.len() {
            content.resize(ix, 0);
        }
        let mut cursor = Cursor::new(&mut *content);
        cursor.set_position(ix as _);
        let n = cursor.write_vectored(bufs)?;

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

struct Link {
    parent: Weak<dyn Node>,

    path: PathBuf,
}

impl Link {
    fn new(parent: Weak<dyn Node>, path: PathBuf) -> Self {
        Self { parent, path }
    }
}

impl Node for Link {
    fn as_any(&self) -> &dyn Any {
        &*self
    }

    fn parent(&self) -> Option<Arc<dyn Node>> {
        self.parent.upgrade()
    }

    fn filetype(&self) -> FileType {
        FileType::SymbolicLink
    }

    fn as_dir(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        follow_symlink: bool,
    ) -> Result<Box<dyn WasiDir>, Error> {
        if !follow_symlink {
            return Err(Error::not_dir());
        }

        self.link_deref(&root)
            .ok_or_else(Error::not_found)
            .and_then(|n| n.as_dir(root, cap, follow_symlink))
    }

    fn as_file(
        self: Arc<Self>,
        root: Option<Arc<Dir>>,
        cap: Capability,
        follow_symlink: bool,
        oflags: OFlags,
        fdflags: FdFlags,
    ) -> Result<Box<dyn WasiFile>, Error> {
        if !follow_symlink {
            return if oflags.intersects(OFlags::DIRECTORY | OFlags::TRUNCATE) || !fdflags.is_empty()
            {
                Err(Error::invalid_argument())
            } else {
                Ok(Box::new(CapAccessor::new(root, self).set_cap(cap)))
            };
        }

        self.link_deref(&root)
            .ok_or_else(Error::not_found)
            .and_then(|n| n.as_file(root, cap, follow_symlink, oflags, fdflags))
    }

    fn as_link(&self) -> Option<PathBuf> {
        Some(self.path.clone())
    }

    fn link_deref(self: Arc<Self>, root: &Option<Arc<Dir>>) -> Option<Arc<dyn Node>> {
        let mut node = self.parent();
        for c in self.path.components() {
            node = match c {
                Component::RootDir => root.clone().map(|v| v as _),
                Component::CurDir => continue,
                Component::ParentDir => node.and_then(|n| n.parent()),
                Component::Normal(name) => node.and_then(|n| n.child(name.to_str().unwrap())),
                Component::Prefix(_) => return None,
            };
        }

        node
    }
}

#[async_trait]
impl WasiFile for CapAccessor<Arc<Link>> {
    fn as_any(&self) -> &dyn Any {
        &*self
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

    async fn set_filestat_size(&self, _size: u64) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn advise(&self, _offset: u64, _len: u64, _advice: Advice) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn allocate(&self, _offset: u64, _len: u64) -> Result<(), Error> {
        Err(Error::not_supported())
    }

    async fn set_times(
        &self,
        _atime: Option<SystemTimeSpec>,
        _mtime: Option<SystemTimeSpec>,
    ) -> Result<(), Error> {
        Err(Error::not_supported())
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
