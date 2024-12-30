#![allow(unused_variables)]

use std::borrow::Cow;
use std::cell::UnsafeCell;
use std::collections::btree_map::BTreeMap;
use std::mem::transmute;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Error as AnyError;
use cap_fs_ext::{FileTypeExt, MetadataExt};
use cfg_if::cfg_if;
use fs_set_times::SetTimes;
use system_interface::fs::FileIoExt;
use wiggle::{GuestError, GuestMemory, GuestPtr, GuestType, Region};

use crate::bindings::types::*;
use crate::context::WasiContext;
use crate::fs_host::Descriptor;
use crate::fs_isolated::NodeItem;
use crate::EMPTY_BUF;

#[derive(Default)]
pub struct P1Items {
    tree: BTreeMap<u32, P1Item>,
}

impl P1Items {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_free(&self) -> u32 {
        assert!(self.tree.len() < u32::MAX as usize, "file descriptor full");
        let Some((&(mut k), _)) = self.tree.last_key_value() else {
            return 0;
        };

        if let Some(k) = k.checked_add(1) {
            debug_assert!(!self.tree.contains_key(&k));
            return k;
        }
        let e = k;
        loop {
            k = k.wrapping_sub(1);
            if !self.tree.contains_key(&k) {
                break k;
            } else if k == e {
                panic!("file descriptor full");
            }
        }
    }

    pub fn register(&mut self, item: P1Item) -> Fd {
        let i = self.next_free();
        self.tree.insert(i, item);
        i.into()
    }

    pub fn unregister(&mut self, fd: Fd) -> Result<P1Item, Error> {
        self.tree
            .remove(&fd.into())
            .ok_or_else(|| Errno::Badf.into())
    }

    pub fn get_mut(&mut self, fd: Fd) -> Result<&mut P1Item, Error> {
        self.tree
            .get_mut(&fd.into())
            .ok_or_else(|| Errno::Badf.into())
    }

    pub fn rename(&mut self, src: Fd, dst: Fd) -> Result<(), Error> {
        let v = self.unregister(src)?;
        self.tree.insert(dst.into(), v);
        Ok(())
    }
}

pub struct P1File {
    preopen: Option<String>,
    cursor: Option<u64>,
    desc: P1Desc,
}

#[non_exhaustive]
pub enum P1Desc {
    IsoFS(crate::fs_isolated::CapWrapper),
    HostFS(crate::fs_host::CapWrapper),
}

pub enum P1DescR<'a> {
    IsoFS(&'a mut crate::fs_isolated::CapWrapper),
    HostFS(&'a mut crate::fs_host::CapWrapper),
}

impl<'a> From<&'a mut P1Desc> for P1DescR<'a> {
    fn from(v: &'a mut P1Desc) -> Self {
        match v {
            P1Desc::IsoFS(v) => Self::IsoFS(v),
            P1Desc::HostFS(v) => Self::HostFS(v),
        }
    }
}

impl From<crate::fs_isolated::CapWrapper> for P1Desc {
    fn from(v: crate::fs_isolated::CapWrapper) -> Self {
        Self::IsoFS(v)
    }
}

impl From<crate::fs_host::CapWrapper> for P1Desc {
    fn from(v: crate::fs_host::CapWrapper) -> Self {
        Self::HostFS(v)
    }
}

impl From<crate::fs_isolated::CapWrapper> for P1File {
    fn from(v: crate::fs_isolated::CapWrapper) -> Self {
        Self::new(v.into())
    }
}

impl From<crate::fs_host::CapWrapper> for P1File {
    fn from(v: crate::fs_host::CapWrapper) -> Self {
        Self::new(v.into())
    }
}

impl P1File {
    #[inline(always)]
    pub fn new(desc: P1Desc) -> Self {
        Self {
            preopen: None,
            cursor: Some(0),
            desc,
        }
    }

    #[inline(always)]
    pub fn with_cursor(desc: P1Desc, cursor: u64) -> Self {
        Self {
            preopen: None,
            cursor: None,
            desc,
        }
    }

    #[inline(always)]
    pub fn with_append(desc: P1Desc) -> Self {
        Self {
            preopen: None,
            cursor: None,
            desc,
        }
    }

    #[inline(always)]
    pub fn with_preopen(desc: P1Desc, preopen: String) -> Self {
        Self {
            preopen: Some(preopen),
            cursor: Some(0),
            desc,
        }
    }

    #[inline(always)]
    pub fn desc(&self) -> &P1Desc {
        &self.desc
    }

    #[inline(always)]
    pub fn get_cursor(&self) -> Option<u64> {
        self.cursor
    }

    #[inline(always)]
    pub fn set_cursor(&mut self, cursor: Option<u64>) {
        self.cursor = cursor;
    }

    #[inline(always)]
    pub fn preopen(&self) -> Option<&str> {
        self.preopen.as_deref()
    }
}

macro_rules! p1item_gen {
    (<$l:lifetime> $($i:ident($t:ty, $t2:ty)),* $(,)?) => {
        #[non_exhaustive]
        pub enum P1Item {
            P1File(Box<P1File>),
            $($i($t)),*
        }

        $(
        impl From<$t> for P1Item {
            fn from(v: $t) -> Self {
                Self::$i(v)
            }
        }
        )*

        #[allow(dead_code)]
        enum FdItem<'a> {
            P1File {
                preopen: Option<&'a str>,
                cursor: Option<&'a mut u64>,
                desc: P1DescR<'a>,
            },
            $($i($t2)),*
        }

        impl P1Items {
            fn get_item(&mut self, fd: Fd) -> Result<FdItem<'_>, Error> {
                Ok(match self.get_mut(fd)? {
                    P1Item::P1File(v) => FdItem::P1File {
                        preopen: v.preopen.as_deref(),
                        cursor: v.cursor.as_mut(),
                        desc: (&mut v.desc).into(),
                    },
                    $(P1Item::$i(v) => FdItem::$i(v)),*
                })
            }
        }
    };
}

p1item_gen! {
    <'a>
    StdinSignal(Arc<crate::stdio::StdinSignal>, &'a crate::stdio::StdinSignal),
    StdoutBp(Arc<crate::stdio::StdoutBypass>, &'a crate::stdio::StdoutBypass),
    StderrBp(Arc<crate::stdio::StderrBypass>, &'a crate::stdio::StderrBypass),
    StdoutLBuf(Arc<crate::stdio::StdoutCbLineBuffered>, &'a crate::stdio::StdoutCbLineBuffered),
    StdoutBBuf(Arc<crate::stdio::StdoutCbBlockBuffered>, &'a crate::stdio::StdoutCbBlockBuffered),
    BoxedRead(Box<dyn Send + Sync + std::io::Read>, &'a mut (dyn Send + Sync + std::io::Read)),
    NullStdio(crate::stdio::NullStdio, &'a mut crate::stdio::NullStdio),
}

struct MemIO<'a, 'b, T> {
    mem: &'a mut GuestMemory<'b>,
    len: Size,
    iov: T,
}

impl<'a, 'b> MemIO<'a, 'b, IovecArray> {
    fn new_read(mem: &'a mut GuestMemory<'b>, iov: IovecArray) -> Result<Self, Error> {
        let mut len: Size = 0;
        for p in iov.iter() {
            len = len.saturating_add(mem.read(p?)?.buf_len);
        }

        Ok(Self { mem, len, iov })
    }

    fn read<T>(
        self,
        mut t: T,
        mut f: impl FnMut(&mut T, Size) -> Result<(Cow<'_, [u8]>, Size), Error>,
    ) -> Result<Size, Error> {
        let Self { mem, mut len, iov } = self;
        if len == 0 {
            return Ok(0);
        }

        let mut iov = iov.iter();
        let mut n = 0;
        let Iovec {
            buf: mut p,
            buf_len: mut blen,
        } = mem.read(
            iov.next()
                .expect("IovecArray ran out before reading complete")?,
        )?;
        while len > 0 {
            let (s, mut l) = f(&mut t, len)?;
            if l == 0 {
                // EOF
                break;
            }

            debug_assert!(
                l <= len,
                "too many bytes returned (asked for {} bytes, got {} bytes)",
                len,
                l
            );
            n += l;
            len -= l;
            let mut s = &s[..];
            while l > 0 {
                // Skip empty iovecs
                while blen == 0 {
                    Iovec {
                        buf: p,
                        buf_len: blen,
                    } = mem.read(
                        iov.next()
                            .expect("IovecArray ran out before reading complete")?,
                    )?;
                }

                let i = l.min(blen);
                let p_ = p;
                l -= i;
                blen -= i;
                p = p.add(i)?;

                s = if let Some((a, b)) = s.split_at_checked(i.try_into()?) {
                    // Copy data
                    mem.copy_from_slice(a, p_.as_array(i))?;
                    b
                } else {
                    // Copy remaining data
                    let mut j = s.len() as Size;
                    mem.copy_from_slice(s, p_.as_array(j))?;

                    // Fill zeros
                    let mut p = p_.add(j)?;
                    j = i - j;
                    while j > 0 {
                        let k = j.min(EMPTY_BUF.len() as _);
                        mem.copy_from_slice(&EMPTY_BUF[..k as usize], p.as_array(k))?;
                        p = p.add(k)?;
                        j -= k;
                    }
                    &[]
                };
            }
        }

        Ok(n)
    }
}

impl<'a, 'b> MemIO<'a, 'b, CiovecArray> {
    fn new_write(mem: &'a mut GuestMemory<'b>, iov: CiovecArray) -> Result<Self, Error> {
        let mut len: Size = 0;
        for p in iov.iter() {
            len = len.saturating_add(mem.read(p?)?.buf_len);
        }

        Ok(Self { mem, len, iov })
    }

    fn write(self, mut f: impl FnMut(&[u8]) -> Result<Size, Error>) -> Result<Size, Error> {
        let Self { mem, iov, len } = self;
        if len == 0 {
            return Ok(0);
        }

        let Some(Ciovec { buf, buf_len }) = iov
            .iter()
            .filter_map(|i| match i.and_then(|i| mem.read(i)) {
                Ok(v) if v.buf_len == 0 => None,
                v => Some(v),
            })
            .next()
            .transpose()?
        else {
            return Ok(0);
        };

        let buf = buf.offset();
        let s = usize::try_from(buf)?;
        let l = usize::try_from(buf_len)?;

        let src = match mem {
            GuestMemory::Unshared(mem) => mem.get(s..).and_then(|v| v.get(..l)),
            GuestMemory::Shared(mem) => mem
                .get(s..)
                .and_then(|v| v.get(..l))
                .map(|v| unsafe { transmute::<&[UnsafeCell<u8>], &[u8]>(v) }),
        };
        match src {
            Some(src) => f(src),
            None => Err(GuestError::PtrOutOfBounds(Region {
                start: buf,
                len: buf_len,
            })
            .into()),
        }
    }
}

fn iso_inode(v: &Arc<crate::fs_isolated::Node>) -> Inode {
    (v.inode() as Inode) ^ (Arc::as_ptr(v) as Inode)
}

fn iso_filetype(v: &crate::fs_isolated::Node) -> Filetype {
    match v.0 {
        NodeItem::Dir(_) => Filetype::Directory,
        NodeItem::File(_) => Filetype::RegularFile,
        NodeItem::Link(_) => Filetype::SymbolicLink,
    }
}

fn to_timestamp(t: SystemTime) -> Timestamp {
    match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(v) => v.as_nanos() as _,
        Err(_) => 0,
    }
}

fn to_filetype(f: cap_std::fs::FileType) -> Filetype {
    if f.is_dir() {
        Filetype::Directory
    } else if f.is_symlink() {
        Filetype::SymbolicLink
    } else if f.is_block_device() {
        Filetype::BlockDevice
    } else if f.is_char_device() {
        Filetype::CharacterDevice
    } else if f.is_file() {
        Filetype::RegularFile
    } else {
        Filetype::Unknown
    }
}

fn set_time(
    dst: &mut SystemTime,
    now: &SystemTime,
    time: Timestamp,
    is_set: bool,
    is_now: bool,
) -> Result<(), Error> {
    *dst = match (is_set, is_now) {
        (true, true) => return Err(Errno::Inval.into()),
        (true, false) => SystemTime::UNIX_EPOCH + Duration::from_nanos(time),
        (false, true) => *now,
        (false, false) => return Ok(()),
    };
    Ok(())
}

fn time_cvt(
    time: Timestamp,
    is_set: bool,
    is_now: bool,
) -> Result<Option<fs_set_times::SystemTimeSpec>, Error> {
    match (is_set, is_now) {
        (true, true) => Err(Errno::Inval.into()),
        (true, false) => Ok(Some(fs_set_times::SystemTimeSpec::Absolute(
            SystemTime::UNIX_EPOCH + Duration::from_nanos(time),
        ))),
        (false, true) => Ok(Some(fs_set_times::SystemTimeSpec::SymbolicNow)),
        (false, false) => Ok(None),
    }
}

impl crate::bindings::wasi_snapshot_preview1::WasiSnapshotPreview1 for WasiContext {
    fn args_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        argv: GuestPtr<GuestPtr<u8>>,
        argv_buf: GuestPtr<u8>,
    ) -> Result<(), Error> {
        let mut l = Some(0);
        for (i, s) in self.args.iter().enumerate() {
            let p = argv_buf.add(l.ok_or(Errno::Overflow)?)?;
            mem.write(argv.add(i.try_into()?)?, p)?;

            let l_ = Size::try_from(s.len())?;
            mem.copy_from_slice(s.as_bytes(), p.as_array(l_))?;
            mem.write(p.add(l_)?, 0)?;

            l = l.and_then(|l| l.checked_add(l_)?.checked_add(1));
        }
        Ok(())
    }

    fn args_sizes_get(&mut self, _: &mut GuestMemory<'_>) -> Result<(Size, Size), Error> {
        let cnt = Size::try_from(self.args.len())?;
        let len = self
            .args
            .iter()
            .try_fold(0 as Size, |a, s| {
                a.checked_add(s.len().try_into().ok()?)?.checked_add(1)
            })
            .ok_or(Errno::Overflow)?;
        Ok((cnt, len))
    }

    fn environ_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        environ: GuestPtr<GuestPtr<u8>>,
        environ_buf: GuestPtr<u8>,
    ) -> Result<(), Error> {
        let mut l = Some(0);
        for (i, (k, v)) in self.envs.iter().enumerate() {
            let mut p = environ_buf.add(l.ok_or(Errno::Overflow)?)?;
            mem.write(environ.add(i.try_into()?)?, p)?;

            let mut l_ = Size::try_from(k.len())?;
            mem.copy_from_slice(k.as_bytes(), p.as_array(l_))?;
            mem.write(p.add(l_)?, b'=')?;

            l = l.and_then(|l| l.checked_add(l_)?.checked_add(1));
            p = environ_buf.add(l.ok_or(Errno::Overflow)?)?;

            l_ = Size::try_from(v.len())?;
            mem.copy_from_slice(v.as_bytes(), p.as_array(l_))?;
            mem.write(p.add(l_)?, 0)?;

            l = l.and_then(|l| l.checked_add(l_)?.checked_add(1));
        }
        Ok(())
    }

    fn environ_sizes_get(&mut self, _: &mut GuestMemory<'_>) -> Result<(Size, Size), Error> {
        let cnt = Size::try_from(self.envs.len())?;
        let len = self
            .envs
            .iter()
            .try_fold(0 as Size, |a, (k, v)| {
                a.checked_add(k.len().try_into().ok()?)?
                    .checked_add(v.len().try_into().ok()?)?
                    .checked_add(2)
            })
            .ok_or(Errno::Overflow)?;
        Ok((cnt, len))
    }

    fn clock_res_get(&mut self, _: &mut GuestMemory<'_>, id: Clockid) -> Result<Timestamp, Error> {
        match id {
            Clockid::Realtime | Clockid::Monotonic => Ok(1000),
            _ => Err(Errno::Badf.into()),
        }
    }

    fn clock_time_get(
        &mut self,
        _: &mut GuestMemory<'_>,
        id: Clockid,
        _: Timestamp,
    ) -> Result<Timestamp, Error> {
        match id {
            Clockid::Realtime => Ok(to_timestamp(SystemTime::now())),
            Clockid::Monotonic => Ok(self.clock.now()),
            _ => Err(Errno::Badf.into()),
        }
    }

    fn fd_advise(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        off: Filesize,
        len: Filesize,
        advice: Advice,
    ) -> Result<(), Error> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => v.file()?.advise(
                off,
                len,
                match advice {
                    Advice::Normal => system_interface::fs::Advice::Normal,
                    Advice::Sequential => system_interface::fs::Advice::Sequential,
                    Advice::Random => system_interface::fs::Advice::Random,
                    Advice::Willneed => system_interface::fs::Advice::WillNeed,
                    Advice::Dontneed => system_interface::fs::Advice::DontNeed,
                    Advice::Noreuse => system_interface::fs::Advice::NoReuse,
                },
            )?,
            FdItem::P1File { .. }
            | FdItem::StdinSignal(_)
            | FdItem::StdoutBp(_)
            | FdItem::StderrBp(_)
            | FdItem::StdoutLBuf(_)
            | FdItem::StdoutBBuf(_)
            | FdItem::BoxedRead(_)
            | FdItem::NullStdio(_) => (),
        }
        Ok(())
    }

    fn fd_allocate(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _: Filesize,
        _: Filesize,
    ) -> Result<(), Error> {
        self.p1_items.get_item(fd)?;
        Err(Errno::Notsup.into())
    }

    fn fd_close(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), Error> {
        self.p1_items.unregister(fd)?;
        Ok(())
    }

    fn fd_datasync(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), Error> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(_),
                ..
            } => (),
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => match &**v.desc() {
                Descriptor::File(v) => v.sync_data().or_else(|e| {
                    cfg_if!{
                        // On windows, `sync_data` uses `FileFlushBuffers` which fails with
                        // `ERROR_ACCESS_DENIED` if the file is not upen for writing. Ignore
                        // this error, for POSIX compatibility.
                        if #[cfg(windows)] {
                            if e.raw_os_error() == Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as _) {
                                Ok(())
                            } else {
                                Err(e)
                            }
                        } else {
                            return Err(e)
                        }
                    }
                })?,
                Descriptor::Dir(v) => v.open(".")?.sync_data()?,
            },
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    fn fd_fdstat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Fdstat, Error> {
        Ok(match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                cursor,
                ..
            } => {
                let mut rights = Rights::FD_DATASYNC
                    | Rights::FD_SYNC
                    | Rights::FD_ADVISE
                    | Rights::FD_ALLOCATE
                    | Rights::FD_FILESTAT_GET
                    | Rights::PATH_LINK_SOURCE
                    | Rights::PATH_LINK_TARGET
                    | Rights::PATH_OPEN;
                if v.node().is_dir() {
                    rights |= Rights::PATH_FILESTAT_GET | Rights::PATH_OPEN;
                    if v.access().is_read() {
                        rights |= Rights::FD_READDIR | Rights::PATH_READLINK;
                    }
                    if v.access().is_write() {
                        rights |= Rights::FD_FDSTAT_SET_FLAGS
                            | Rights::FD_FILESTAT_SET_SIZE
                            | Rights::FD_FILESTAT_SET_TIMES
                            | Rights::PATH_FILESTAT_SET_SIZE
                            | Rights::PATH_FILESTAT_SET_TIMES
                            | Rights::PATH_RENAME_SOURCE
                            | Rights::PATH_RENAME_TARGET
                            | Rights::PATH_UNLINK_FILE
                            | Rights::PATH_REMOVE_DIRECTORY;
                    }
                }
                if v.node().is_link() {
                    if v.access().is_read() {
                        rights |= Rights::PATH_READLINK;
                    }
                    if v.access().is_write() {
                        rights |= Rights::PATH_SYMLINK;
                    }
                }
                if v.node().is_file() {
                    rights |= Rights::FD_SEEK | Rights::FD_TELL;
                    if v.access().is_read() {
                        rights |= Rights::FD_READ;
                    }
                    if v.access().is_write() {
                        rights |= Rights::FD_WRITE
                            | Rights::FD_FDSTAT_SET_FLAGS
                            | Rights::FD_FILESTAT_SET_SIZE
                            | Rights::FD_FILESTAT_SET_TIMES;
                    }
                }

                Fdstat {
                    fs_filetype: iso_filetype(v.node()),
                    fs_flags: if v.node().is_file() && cursor.is_none() {
                        Fdflags::APPEND
                    } else {
                        Fdflags::empty()
                    },
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                cursor,
                ..
            } => {
                let f = match &**v.desc() {
                    Descriptor::File(v) => v.metadata(),
                    Descriptor::Dir(v) => v.dir_metadata(),
                }?
                .file_type();
                let mut rights = Rights::FD_DATASYNC
                    | Rights::FD_SYNC
                    | Rights::FD_ADVISE
                    | Rights::FD_ALLOCATE
                    | Rights::FD_FILESTAT_GET
                    | Rights::PATH_LINK_SOURCE
                    | Rights::PATH_LINK_TARGET
                    | Rights::PATH_OPEN;
                match **v.desc() {
                    Descriptor::Dir(_) => {
                        rights |= Rights::PATH_FILESTAT_GET | Rights::PATH_OPEN;
                        if v.access().is_read() {
                            rights |= Rights::FD_READDIR | Rights::PATH_READLINK;
                        }
                        if v.access().is_write() {
                            rights |= Rights::FD_FDSTAT_SET_FLAGS
                                | Rights::FD_FILESTAT_SET_SIZE
                                | Rights::FD_FILESTAT_SET_TIMES
                                | Rights::PATH_FILESTAT_SET_SIZE
                                | Rights::PATH_FILESTAT_SET_TIMES
                                | Rights::PATH_RENAME_SOURCE
                                | Rights::PATH_RENAME_TARGET
                                | Rights::PATH_UNLINK_FILE
                                | Rights::PATH_REMOVE_DIRECTORY;
                        }
                    }
                    Descriptor::File(_) => {
                        rights |= Rights::FD_SEEK | Rights::FD_TELL;
                        if v.access().is_read() {
                            rights |= Rights::FD_READ;
                        }
                        if v.access().is_write() {
                            rights |= Rights::FD_WRITE
                                | Rights::FD_FDSTAT_SET_FLAGS
                                | Rights::FD_FILESTAT_SET_SIZE
                                | Rights::FD_FILESTAT_SET_TIMES;
                        }
                    }
                }

                Fdstat {
                    fs_filetype: to_filetype(f),
                    fs_flags: if matches!(**v.desc(), Descriptor::File(_)) && cursor.is_none() {
                        Fdflags::APPEND
                    } else {
                        Fdflags::empty()
                    },
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
            FdItem::StdinSignal(_) | FdItem::BoxedRead(_) => {
                let rights = Rights::FD_READ;
                Fdstat {
                    fs_filetype: Filetype::Unknown,
                    fs_flags: Fdflags::empty(),
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
            FdItem::StdoutBp(_)
            | FdItem::StderrBp(_)
            | FdItem::StdoutLBuf(_)
            | FdItem::StdoutBBuf(_) => {
                let rights = Rights::FD_WRITE;
                Fdstat {
                    fs_filetype: Filetype::Unknown,
                    fs_flags: Fdflags::empty(),
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
            FdItem::NullStdio(_) => {
                let rights = Rights::FD_READ | Rights::FD_WRITE;
                Fdstat {
                    fs_filetype: Filetype::Unknown,
                    fs_flags: Fdflags::empty(),
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
        })
    }

    fn fd_fdstat_set_flags(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _: Fdflags,
    ) -> Result<(), Error> {
        self.p1_items.get_item(fd)?;
        Err(Errno::Notsup.into())
    }

    fn fd_fdstat_set_rights(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _: Rights,
        _: Rights,
    ) -> Result<(), Error> {
        self.p1_items.get_item(fd)?;
        Err(Errno::Notsup.into())
    }

    fn fd_filestat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Filestat, Error> {
        Ok(match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                ..
            } => {
                fn map_stamp(
                    stamp: &crate::fs_isolated::Timestamp,
                ) -> (Timestamp, Timestamp, Timestamp) {
                    (
                        to_timestamp(stamp.ctime),
                        to_timestamp(stamp.mtime),
                        to_timestamp(stamp.atime),
                    )
                }

                let (filetype, size, (ctim, mtim, atim)) = match &v.node().0 {
                    NodeItem::Dir(v) => {
                        let v = v.lock();
                        (Filetype::Directory, v.len(), map_stamp(v.stamp()))
                    }
                    NodeItem::File(v) => {
                        let v = v.lock();
                        (Filetype::RegularFile, v.len(), map_stamp(v.stamp()))
                    }
                    NodeItem::Link(v) => {
                        let v = v.read();
                        (Filetype::SymbolicLink, v.len(), map_stamp(v.stamp()))
                    }
                };

                Filestat {
                    dev: 127,
                    ino: iso_inode(v.node()),
                    nlink: 0,
                    size: size as _,
                    filetype,
                    ctim,
                    mtim,
                    atim,
                }
            }
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => {
                fn f(v: std::io::Result<cap_std::time::SystemTime>) -> Timestamp {
                    v.ok().map_or(0, |v| to_timestamp(v.into_std()))
                }

                let m = match &**v.desc() {
                    Descriptor::File(v) => v.metadata(),
                    Descriptor::Dir(v) => v.dir_metadata(),
                }?;
                let filetype = to_filetype(m.file_type());
                let nlink = m.nlink();
                let size = m.len();
                let ctim = f(m.created());
                let mtim = f(m.modified());
                let atim = f(m.accessed());
                let inode = crate::fs_host::CapWrapper::meta_hash(m, &self.hasher);

                Filestat {
                    dev: 1,
                    ino: inode.lower ^ inode.upper,
                    size: size as _,
                    filetype,
                    nlink,
                    ctim,
                    mtim,
                    atim,
                }
            }
            FdItem::StdinSignal(_)
            | FdItem::BoxedRead(_)
            | FdItem::StdoutBp(_)
            | FdItem::StderrBp(_)
            | FdItem::StdoutLBuf(_)
            | FdItem::StdoutBBuf(_)
            | FdItem::NullStdio(_) => Filestat {
                dev: 0,
                ino: 0,
                filetype: Filetype::Unknown,
                size: 0,
                nlink: 0,
                ctim: 0,
                mtim: 0,
                atim: 0,
            },
        })
    }

    fn fd_filestat_set_size(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        size: Filesize,
    ) -> Result<(), Error> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                ..
            } => v.resize(size.try_into().map_err(AnyError::from)?)?,
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => v.write()?.file()?.set_len(size)?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    fn fd_filestat_set_times(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        atim: Timestamp,
        mtim: Timestamp,
        fst_flags: Fstflags,
    ) -> Result<(), Error> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                ..
            } => v.set_time(|stamp| -> Result<_, Error> {
                let now = SystemTime::now();
                set_time(
                    &mut stamp.mtime,
                    &now,
                    mtim,
                    fst_flags.contains(Fstflags::MTIM),
                    fst_flags.contains(Fstflags::MTIM_NOW),
                )?;
                set_time(
                    &mut stamp.atime,
                    &now,
                    atim,
                    fst_flags.contains(Fstflags::ATIM),
                    fst_flags.contains(Fstflags::ATIM_NOW),
                )?;
                Ok(())
            })?,
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => {
                let atime = time_cvt(
                    atim,
                    fst_flags.contains(Fstflags::ATIM),
                    fst_flags.contains(Fstflags::ATIM_NOW),
                )?;
                let mtime = time_cvt(
                    mtim,
                    fst_flags.contains(Fstflags::MTIM),
                    fst_flags.contains(Fstflags::MTIM_NOW),
                )?;
                match &**v.write()?.desc() {
                    Descriptor::File(v) => v.set_times(atime, mtime),
                    Descriptor::Dir(v) => SetTimes::set_times(v, atime, mtime),
                }?
            }
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    fn fd_pread(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: IovecArray,
        offset: Filesize,
    ) -> Result<Size, Error> {
        let memio = MemIO::new_read(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                ..
            } => {
                v.access().read_or_err()?;
                let mut off = usize::try_from(offset)?;
                memio.read(v.node().file().ok_or(Errno::Isdir)?, |v, len| {
                    let (s, l) = v.read(len.try_into()?, off);
                    off += l;
                    Ok((s.into(), l as Size))
                })
            }
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => {
                // Only try to read once per syscall.
                let mut has_read = false;
                memio.read(v.read()?.file()?, |v, len| {
                    if len == 0 || has_read {
                        return Ok(((&[]).into(), 0));
                    }

                    let mut ret = vec![0; EMPTY_BUF.len().min(len.try_into()?)];
                    let l = crate::fs_host::CapWrapper::read_at(v, &mut ret, offset)?;
                    ret.truncate(l);
                    has_read = true;
                    Ok((ret.into(), l as Size))
                })
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    fn fd_prestat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Prestat, Error> {
        if let FdItem::P1File {
            preopen: Some(s), ..
        } = self.p1_items.get_item(fd)?
        {
            Ok(Prestat::Dir(PrestatDir {
                pr_name_len: s.len().try_into()?,
            }))
        } else {
            Err(Errno::Badf.into())
        }
    }

    fn fd_prestat_dir_name(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<u8>,
        path_len: Size,
    ) -> Result<(), Error> {
        if let FdItem::P1File {
            preopen: Some(s), ..
        } = self.p1_items.get_item(fd)?
        {
            if s.len() > usize::try_from(path_len)? {
                return Err(Errno::Nametoolong.into());
            }

            mem.copy_from_slice(s.as_bytes(), path.as_array(s.len().try_into()?))?;
            Ok(())
        } else {
            Err(Errno::Notdir.into())
        }
    }

    fn fd_pwrite(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: CiovecArray,
        mut offset: Filesize,
    ) -> Result<Size, Error> {
        let memio = MemIO::new_write(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                ..
            } => {
                v.access().write_or_err()?;
                let mut v = v.node().file().ok_or(Errno::Isdir)?;
                let mut off = usize::try_from(offset)?;
                memio.write(|s| {
                    v.write(s, off)?;
                    off += s.len();
                    Ok(s.len() as Size)
                })
            }
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => {
                let v = v.write()?.file()?;
                memio.write(|s| {
                    let l = v.write_at(s, offset)?;
                    offset += l as Filesize;
                    Ok(l as Size)
                })
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    fn fd_read(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: IovecArray,
    ) -> Result<Size, Error> {
        let memio = MemIO::new_read(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                cursor,
                ..
            } => {
                v.access().read_or_err()?;
                let v = v.node().file().ok_or(Errno::Isdir)?;
                let Some(c) = cursor else { return Ok(0) };
                let old = *c;

                let r = memio.read(v, |v, len| {
                    let (s, l) = v.read(len.try_into()?, (*c).try_into().unwrap_or(usize::MAX));
                    *c += l as u64;
                    Ok((s.into(), l as Size))
                });
                if r.is_err() {
                    *c = old;
                }
                r
            }
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                cursor,
                ..
            } => {
                // Only try to read once per syscall.
                let mut has_read = false;
                let Some(c) = cursor else { return Ok(0) };
                let old = *c;

                let r = memio.read(v.read()?.file()?, |v, len| {
                    if len == 0 || has_read {
                        return Ok(((&[]).into(), 0));
                    }

                    let mut ret = vec![0; EMPTY_BUF.len().min(len.try_into()?)];
                    let l = crate::fs_host::CapWrapper::read_at(v, &mut ret, *c as _)?;
                    ret.truncate(l);
                    *c += l as u64;
                    has_read = true;
                    Ok((ret.into(), l as Size))
                });
                if r.is_err() {
                    *c = old;
                }
                r
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    fn fd_readdir(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        buf: GuestPtr<u8>,
        buf_len: Size,
        cookie: Dircookie,
    ) -> Result<Size, Error> {
        fn write_dirent(buf: &mut [u8], d: Dirent) {
            *buf[Dirent::offset_of_d_next() as usize..]
                .split_first_chunk_mut()
                .unwrap()
                .0 = d.d_next.to_le_bytes();
            *buf[Dirent::offset_of_d_ino() as usize..]
                .split_first_chunk_mut()
                .unwrap()
                .0 = d.d_ino.to_le_bytes();
            *buf[Dirent::offset_of_d_namlen() as usize..]
                .split_first_chunk_mut()
                .unwrap()
                .0 = d.d_namlen.to_le_bytes();
            buf[Dirent::offset_of_d_type() as usize] = d.d_type.into();
        }

        fn f<S: AsRef<str>>(
            mem: &mut GuestMemory<'_>,
            buf: GuestPtr<u8>,
            buf_len: Size,
            cookie: Dircookie,
            cur_ino: Inode,
            it: impl IntoIterator<Item = Result<(Inode, Filetype, S), Error>>,
        ) -> Result<Size, Error> {
            let buf = buf.offset();
            let s = usize::try_from(buf)?;
            let l = usize::try_from(buf_len)?;
            let b = match mem {
                GuestMemory::Unshared(mem) => mem.get_mut(s..).and_then(|v| v.get_mut(..l)),
                GuestMemory::Shared(mem) => mem.get(s..).and_then(|v| v.get(..l)).map(|v| {
                    // SAFETY: Responsibility for shared access exclusivity is on guest.
                    #[allow(mutable_transmutes)]
                    unsafe {
                        transmute::<&[UnsafeCell<u8>], &mut [u8]>(v)
                    }
                }),
            };
            let Some(mut buf) = b else {
                return Err(GuestError::PtrOutOfBounds(Region {
                    start: buf,
                    len: buf_len,
                })
                .into());
            };

            let entry_size = Dirent::guest_size() as usize;

            let mut i = entry_size + 1;
            if cookie == 0 {
                let Some((b, r)) = buf.split_at_mut_checked(i) else {
                    return Ok(0);
                };
                write_dirent(
                    b,
                    Dirent {
                        d_next: 1,
                        d_ino: cur_ino,
                        d_namlen: 1,
                        d_type: Filetype::Directory,
                    },
                );
                b[entry_size] = b'.';
                buf = r;
            }

            if cookie <= 1 {
                let Some((b, r)) = buf.split_at_mut_checked(entry_size + 2) else {
                    return Ok(i as _);
                };
                write_dirent(
                    b,
                    Dirent {
                        d_next: 2,
                        d_ino: cur_ino,
                        d_namlen: 2,
                        d_type: Filetype::Directory,
                    },
                );
                b[entry_size..entry_size + 2].copy_from_slice(b"..");
                (i, buf) = (i + b.len(), r);
            }

            for (v, next) in it.into_iter().zip(3 as Dircookie..) {
                let (ino, ft, name) = v?;
                if cookie >= next {
                    continue;
                }

                let name = name.as_ref().as_bytes();
                let nl = Size::try_from(name.len()).unwrap_or(Size::MIN) as usize;
                let name = &name[..nl];
                let Some((b, r)) = entry_size
                    .checked_add(nl)
                    .and_then(|i| buf.split_at_mut_checked(i))
                else {
                    break;
                };
                write_dirent(
                    b,
                    Dirent {
                        d_next: next,
                        d_ino: ino,
                        d_namlen: nl as _,
                        d_type: ft,
                    },
                );
                b[entry_size..].copy_from_slice(name);
                (i, buf) = (i + b.len(), r);
            }

            Ok(i as _)
        }

        match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                ..
            } => f(
                mem,
                buf,
                buf_len,
                cookie,
                iso_inode(v.node()),
                v.read_directory()?.map(|v| {
                    v.map(|(k, v)| (v.inode() as Inode, iso_filetype(&v), k))
                        .map_err(Error::from)
                }),
            ),
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                ..
            } => {
                let inode = v.metadata_hash(&self.hasher)?;
                f(
                    mem,
                    buf,
                    buf_len,
                    cookie,
                    inode.lower ^ inode.upper,
                    v.read_dir()?.map(|v| {
                        let v = v?;
                        let m = v.metadata()?;
                        let name = v.file_name().into_string().ok().ok_or(Errno::Inval)?;
                        let ft = to_filetype(m.file_type());
                        let inode = crate::fs_host::CapWrapper::meta_hash(m, &self.hasher);
                        Ok((inode.lower ^ inode.upper, ft, name))
                    }),
                )
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    fn fd_renumber(&mut self, _: &mut GuestMemory<'_>, fd: Fd, to: Fd) -> Result<(), Error> {
        self.p1_items.rename(fd, to)
    }

    fn fd_seek(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        offset: Filedelta,
        whence: Whence,
    ) -> Result<Filesize, Error> {
        Ok(match self.p1_items.get_item(fd)? {
            FdItem::P1File {
                desc: P1DescR::IsoFS(v),
                cursor: Some(c),
                ..
            } => {
                let l = v.node().file().ok_or(Errno::Isdir)?.len() as u64;
                *c = match whence {
                    Whence::Set => offset.try_into().ok(),
                    Whence::Cur => c.checked_add_signed(offset),
                    Whence::End => l.checked_add_signed(offset),
                }
                .ok_or(Errno::Inval)?;
                *c as _
            }
            FdItem::P1File {
                desc: P1DescR::HostFS(v),
                cursor: Some(c),
                ..
            } => {
                let v = v.file()?;
                *c = match whence {
                    Whence::Set => offset.try_into().ok(),
                    Whence::Cur => c.checked_add_signed(offset),
                    Whence::End => v.metadata()?.len().checked_add_signed(offset),
                }
                .ok_or(Errno::Inval)?;
                *c as _
            }
            _ => return Err(Errno::Badf.into()),
        })
    }

    fn fd_sync(&mut self, mem: &mut GuestMemory<'_>, fd: Fd) -> Result<(), Error> {
        todo!()
    }

    fn fd_tell(&mut self, mem: &mut GuestMemory<'_>, fd: Fd) -> Result<Filesize, Error> {
        todo!()
    }

    fn fd_write(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: CiovecArray,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn path_create_directory(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn path_filestat_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        flags: Lookupflags,
        path: GuestPtr<str>,
    ) -> Result<Filestat, Error> {
        todo!()
    }

    fn path_filestat_set_times(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        flags: Lookupflags,
        path: GuestPtr<str>,
        atim: Timestamp,
        mtim: Timestamp,
        fst_flags: Fstflags,
    ) -> Result<(), Error> {
        todo!()
    }

    fn path_link(
        &mut self,
        mem: &mut GuestMemory<'_>,
        old_fd: Fd,
        old_flags: Lookupflags,
        old_path: GuestPtr<str>,
        new_fd: Fd,
        new_path: GuestPtr<str>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn path_open(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        dirflags: Lookupflags,
        path: GuestPtr<str>,
        oflags: Oflags,
        fs_rights_base: Rights,
        fs_rights_inheriting: Rights,
        fdflags: Fdflags,
    ) -> Result<Fd, Error> {
        todo!()
    }

    fn path_readlink(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
        buf: GuestPtr<u8>,
        buf_len: Size,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn path_remove_directory(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn path_rename(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        old_path: GuestPtr<str>,
        new_fd: Fd,
        new_path: GuestPtr<str>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn path_symlink(
        &mut self,
        mem: &mut GuestMemory<'_>,
        old_path: GuestPtr<str>,
        fd: Fd,
        new_path: GuestPtr<str>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn path_unlink_file(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn poll_oneoff(
        &mut self,
        mem: &mut GuestMemory<'_>,
        in_: GuestPtr<Subscription>,
        out: GuestPtr<Event>,
        nsubscriptions: Size,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn proc_exit(&mut self, mem: &mut GuestMemory<'_>, rval: Exitcode) -> AnyError {
        todo!()
    }

    fn proc_raise(&mut self, mem: &mut GuestMemory<'_>, sig: Signal) -> Result<(), Error> {
        todo!()
    }

    fn sched_yield(&mut self, mem: &mut GuestMemory<'_>) -> Result<(), Error> {
        todo!()
    }

    fn random_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        buf: GuestPtr<u8>,
        buf_len: Size,
    ) -> Result<(), Error> {
        todo!()
    }

    fn sock_accept(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        flags: Fdflags,
    ) -> Result<Fd, Error> {
        todo!()
    }

    fn sock_recv(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        ri_data: IovecArray,
        ri_flags: Riflags,
    ) -> Result<(Size, Roflags), Error> {
        todo!()
    }

    fn sock_send(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        si_data: CiovecArray,
        si_flags: Siflags,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn sock_shutdown(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        how: Sdflags,
    ) -> Result<(), Error> {
        todo!()
    }
}
