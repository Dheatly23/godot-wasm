#![allow(unused_variables)]

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Error as AnyError;
use cap_fs_ext::{FileTypeExt, MetadataExt};
use cfg_if::cfg_if;
use fs_set_times::SetTimes;
use system_interface::fs::FileIoExt;
use wiggle::{GuestMemory, GuestPtr};

use crate::bindings::types::*;
use crate::context::WasiContext;
use crate::fs_host::Descriptor;
use crate::fs_isolated::NodeItem;
use crate::items::{Item, MaybeBorrowMut, ResItem};
use crate::EMPTY_BUF;

struct ResOwned<T>(T);

impl<T: ResItem> ResItem for ResOwned<T> {
    type ItemOut<'a> = T::ItemOut<'a>;
    type ItemOutRef<'a> = T::ItemOutRef<'a>;

    #[inline(always)]
    fn is_owned(&self) -> bool {
        true
    }

    #[inline(always)]
    fn id(&self) -> u32 {
        self.0.id()
    }

    #[inline(always)]
    fn from_item<'a>(item: Item) -> Option<T::ItemOut<'a>> {
        T::from_item(item)
    }

    #[inline(always)]
    fn from_item_ref(item: &Item) -> Option<T::ItemOutRef<'_>> {
        T::from_item_ref(item)
    }

    #[inline(always)]
    fn from_item_mut(item: &mut Item) -> Option<T::ItemOut<'_>> {
        T::from_item_mut(item)
    }
}

#[allow(clippy::enum_variant_names, dead_code)]
pub(crate) enum FdItem<'a> {
    IsoFSNode(MaybeBorrowMut<'a, Box<crate::fs_isolated::CapWrapper>>),
    HostFSDesc(MaybeBorrowMut<'a, Box<crate::fs_host::CapWrapper>>),
    StdinSignal(MaybeBorrowMut<'a, Arc<crate::stdio::StdinSignal>>),
    StdoutBp(MaybeBorrowMut<'a, Arc<crate::stdio::StdoutBypass>>),
    StderrBp(MaybeBorrowMut<'a, Arc<crate::stdio::StderrBypass>>),
    StdoutLBuf(MaybeBorrowMut<'a, Arc<crate::stdio::StdoutCbLineBuffered>>),
    StdoutBBuf(MaybeBorrowMut<'a, Arc<crate::stdio::StdoutCbBlockBuffered>>),
    BoxedRead(MaybeBorrowMut<'a, Box<dyn Send + Sync + std::io::Read>>),
    NullStdio(MaybeBorrowMut<'a, crate::stdio::NullStdio>),
}

#[allow(clippy::enum_variant_names, clippy::borrowed_box, dead_code)]
pub(crate) enum FdItemR<'a> {
    IsoFSNode(&'a Box<crate::fs_isolated::CapWrapper>),
    HostFSDesc(&'a Box<crate::fs_host::CapWrapper>),
    StdinSignal(&'a Arc<crate::stdio::StdinSignal>),
    StdoutBp(&'a Arc<crate::stdio::StdoutBypass>),
    StderrBp(&'a Arc<crate::stdio::StderrBypass>),
    StdoutLBuf(&'a Arc<crate::stdio::StdoutCbLineBuffered>),
    StdoutBBuf(&'a Arc<crate::stdio::StdoutCbBlockBuffered>),
    BoxedRead(&'a Box<dyn Send + Sync + std::io::Read>),
    NullStdio(&'a crate::stdio::NullStdio),
}

impl ResItem for Fd {
    type ItemOut<'a> = FdItem<'a>;
    type ItemOutRef<'a> = FdItemR<'a>;

    #[inline(always)]
    fn is_owned(&self) -> bool {
        false
    }

    #[inline(always)]
    fn id(&self) -> u32 {
        unsafe { self.inner() }
    }

    fn from_item<'a>(item: Item) -> Option<Self::ItemOut<'a>> {
        Some(match item {
            Item::IsoFSNode(v) => FdItem::IsoFSNode(v.into()),
            Item::HostFSDesc(v) => FdItem::HostFSDesc(v.into()),
            Item::StdinSignal(v) => FdItem::StdinSignal(v.into()),
            Item::StdoutBp(v) => FdItem::StdoutBp(v.into()),
            Item::StderrBp(v) => FdItem::StderrBp(v.into()),
            Item::StdoutLBuf(v) => FdItem::StdoutLBuf(v.into()),
            Item::StdoutBBuf(v) => FdItem::StdoutBBuf(v.into()),
            Item::BoxedRead(v) => FdItem::BoxedRead(v.into()),
            Item::NullStdio(v) => FdItem::NullStdio(v.into()),
            _ => return None,
        })
    }

    fn from_item_ref(item: &Item) -> Option<Self::ItemOutRef<'_>> {
        Some(match item {
            Item::IsoFSNode(v) => FdItemR::IsoFSNode(v),
            Item::HostFSDesc(v) => FdItemR::HostFSDesc(v),
            Item::StdinSignal(v) => FdItemR::StdinSignal(v),
            Item::StdoutBp(v) => FdItemR::StdoutBp(v),
            Item::StderrBp(v) => FdItemR::StderrBp(v),
            Item::StdoutLBuf(v) => FdItemR::StdoutLBuf(v),
            Item::StdoutBBuf(v) => FdItemR::StdoutBBuf(v),
            Item::BoxedRead(v) => FdItemR::BoxedRead(v),
            Item::NullStdio(v) => FdItemR::NullStdio(v),
            _ => return None,
        })
    }

    fn from_item_mut(item: &mut Item) -> Option<Self::ItemOut<'_>> {
        Some(match item {
            Item::IsoFSNode(v) => FdItem::IsoFSNode(v.into()),
            Item::HostFSDesc(v) => FdItem::HostFSDesc(v.into()),
            Item::StdinSignal(v) => FdItem::StdinSignal(v.into()),
            Item::StdoutBp(v) => FdItem::StdoutBp(v.into()),
            Item::StderrBp(v) => FdItem::StderrBp(v.into()),
            Item::StdoutLBuf(v) => FdItem::StdoutLBuf(v.into()),
            Item::StdoutBBuf(v) => FdItem::StdoutBBuf(v.into()),
            Item::BoxedRead(v) => FdItem::BoxedRead(v.into()),
            Item::NullStdio(v) => FdItem::NullStdio(v.into()),
            _ => return None,
        })
    }
}

struct MemIO<'a, 'b> {
    mem: &'a mut GuestMemory<'b>,
    len: Size,
    off: Size,
    iov: IovecArray,
}

impl<'a, 'b> MemIO<'a, 'b> {
    fn new(mem: &'a mut GuestMemory<'b>, iov: IovecArray) -> Result<Self, Error> {
        let mut len: Size = 0;
        for p in iov.iter() {
            len = len.saturating_add(mem.read(p?)?.buf_len);
        }

        Ok(Self {
            mem,
            len,
            iov,
            off: 0,
        })
    }

    fn read<T>(
        &mut self,
        mut t: T,
        mut f: impl FnMut(&mut T, Size) -> Result<(Cow<'_, [u8]>, Size), Error>,
    ) -> Result<Size, Error> {
        if self.len == 0 {
            return Ok(0);
        }

        let mut n = 0;
        let Iovec {
            buf: mut p,
            buf_len: mut blen,
        } = self.mem.read(self.iov.as_ptr())?;
        blen -= self.off;
        p = p.add(self.off)?;
        while self.len > 0 {
            let (s, mut l) = f(&mut t, self.len)?;
            if l == 0 {
                // EOF
                break;
            }

            debug_assert!(
                l <= self.len,
                "too many bytes returned (asked for {} bytes, got {} bytes)",
                self.len,
                l
            );
            n += l;
            self.len -= l;
            let mut s = &s[..];
            while l > 0 {
                // Skip empty iovecs
                while blen == 0 {
                    debug_assert_ne!(self.iov.len(), 0, "IO slice ran out before length ran out (remaining length: {}, writing length: {})", self.len, l);
                    self.iov = self.iov.as_ptr().add(1)?.as_array(self.iov.len() - 1);
                    self.off = 0;
                    Iovec {
                        buf: p,
                        buf_len: blen,
                    } = self.mem.read(self.iov.as_ptr())?;
                }

                let i = l.min(blen);
                let p_ = p;
                l -= i;
                blen -= i;
                self.off += i;
                p = p.add(i)?;

                s = if let Some((a, b)) = s.split_at_checked(i.try_into()?) {
                    // Copy data
                    self.mem.copy_from_slice(a, p_.as_array(i))?;
                    b
                } else {
                    // Copy remaining data
                    let mut j = s.len() as Size;
                    self.mem.copy_from_slice(s, p_.as_array(j))?;

                    // Fill zeros
                    let mut p = p_.add(j)?;
                    j = i - j;
                    while j > 0 {
                        let k = j.min(EMPTY_BUF.len() as _);
                        self.mem
                            .copy_from_slice(&EMPTY_BUF[..k as usize], p.as_array(k))?;
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
        match self.items.get_item(fd)? {
            FdItem::IsoFSNode(_)
            | FdItem::StdinSignal(_)
            | FdItem::StdoutBp(_)
            | FdItem::StderrBp(_)
            | FdItem::StdoutLBuf(_)
            | FdItem::StdoutBBuf(_)
            | FdItem::BoxedRead(_)
            | FdItem::NullStdio(_) => (),
            FdItem::HostFSDesc(v) => v.file()?.advise(
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
        self.items.get_item(fd)?;
        Err(Errno::Notsup.into())
    }

    fn fd_close(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), Error> {
        self.items.get_item(ResOwned(fd))?;
        Ok(())
    }

    fn fd_datasync(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), Error> {
        match self.items.get_item(fd)? {
            FdItem::IsoFSNode(_) => (),
            FdItem::HostFSDesc(v) => match &**v.desc() {
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
            _ => return Err(Errno::Inval.into()),
        }
        Ok(())
    }

    fn fd_fdstat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Fdstat, Error> {
        Ok(match self.items.get_item(fd)? {
            FdItem::IsoFSNode(v) => {
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
                    fs_filetype: match v.node().0 {
                        NodeItem::Dir(_) => Filetype::Directory,
                        NodeItem::File(_) => Filetype::RegularFile,
                        NodeItem::Link(_) => Filetype::SymbolicLink,
                    },
                    fs_flags: if v.node().is_file() && v.cursor.is_none() {
                        Fdflags::APPEND
                    } else {
                        Fdflags::empty()
                    },
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
            FdItem::HostFSDesc(v) => {
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
                    fs_flags: if matches!(**v.desc(), Descriptor::File(_)) && v.cursor.is_none() {
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
        self.items.get_item(fd)?;
        Err(Errno::Notsup.into())
    }

    fn fd_fdstat_set_rights(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _: Rights,
        _: Rights,
    ) -> Result<(), Error> {
        self.items.get_item(fd)?;
        Err(Errno::Notsup.into())
    }

    fn fd_filestat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Filestat, Error> {
        Ok(match self.items.get_item(fd)? {
            FdItem::IsoFSNode(v) => {
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
                        (Filetype::Directory, v.len(), map_stamp(v.stamp()))
                    }
                    NodeItem::Link(v) => {
                        let v = v.read();
                        (Filetype::Directory, v.len(), map_stamp(v.stamp()))
                    }
                };

                Filestat {
                    dev: 127,
                    ino: (v.node().inode() as u64)
                        .wrapping_mul(9973)
                        .wrapping_add(Arc::as_ptr(v.node()) as u64),
                    nlink: 0,
                    size: size as _,
                    filetype,
                    ctim,
                    mtim,
                    atim,
                }
            }
            FdItem::HostFSDesc(v) => {
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
        match self.items.get_item(fd)? {
            FdItem::IsoFSNode(v) => v.resize(size.try_into().map_err(AnyError::from)?)?,
            FdItem::HostFSDesc(v) => v.write()?.file()?.set_len(size)?,
            _ => return Err(Errno::Inval.into()),
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
        match self.items.get_item(fd)? {
            FdItem::IsoFSNode(v) => v.set_time(|stamp| -> Result<_, Error> {
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
            FdItem::HostFSDesc(v) => {
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
            _ => return Err(Errno::Inval.into()),
        }
        Ok(())
    }

    fn fd_pread(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: IovecArray,
        mut offset: Filesize,
    ) -> Result<Size, Error> {
        let mut memio = MemIO::new(mem, iovs)?;

        match self.items.get_item(fd)? {
            FdItem::IsoFSNode(v) => {
                v.access().read_or_err()?;
                let mut off = usize::try_from(offset)?;
                memio.read(v.node().file().ok_or(Errno::Isdir)?, |v, len| {
                    let (s, l) = v.read(len.try_into()?, off);
                    off += l;
                    Ok((s.into(), l as Size))
                })
            }
            FdItem::HostFSDesc(v) => memio.read(
                (
                    v.read()?.file()?,
                    vec![0u8; usize::try_from(memio.len)?.min(EMPTY_BUF.len())],
                ),
                |(v, ret), len| {
                    let i = ret.len().min(len.try_into()?);
                    let ret = &mut ret[..i];
                    let l = crate::fs_host::CapWrapper::read_at(v, &mut *ret, offset)?;
                    offset += l as Filesize;
                    Ok(((&ret[..l]).into(), l as Size))
                },
            ),
            _ => Err(Errno::Inval.into()),
        }
    }

    fn fd_prestat_get(&mut self, mem: &mut GuestMemory<'_>, fd: Fd) -> Result<Prestat, Error> {
        todo!()
    }

    fn fd_prestat_dir_name(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<u8>,
        path_len: Size,
    ) -> Result<(), Error> {
        todo!()
    }

    fn fd_pwrite(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: CiovecArray,
        offset: Filesize,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn fd_read(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: IovecArray,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn fd_readdir(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        buf: GuestPtr<u8>,
        buf_len: Size,
        cookie: Dircookie,
    ) -> Result<Size, Error> {
        todo!()
    }

    fn fd_renumber(&mut self, mem: &mut GuestMemory<'_>, fd: Fd, to: Fd) -> Result<(), Error> {
        todo!()
    }

    fn fd_seek(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        offset: Filedelta,
        whence: Whence,
    ) -> Result<Filesize, Error> {
        todo!()
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
