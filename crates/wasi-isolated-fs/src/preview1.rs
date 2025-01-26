use std::borrow::Cow;
use std::cell::UnsafeCell;
use std::collections::btree_map::{BTreeMap, Entry, VacantEntry};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::ErrorKind;
use std::mem::transmute;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Error as AnyError, Result as AnyResult};
use camino::{Utf8Path, Utf8PathBuf};
use cap_fs_ext::{DirExt, FileTypeExt, MetadataExt, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use fs_set_times::SetTimes;
use rand::Rng;
use smallvec::SmallVec;
use system_interface::fs::FileIoExt;
use tracing::{debug, debug_span, info, instrument, warn, Level};
use wiggle::{GuestError, GuestMemory, GuestPtr, GuestType, Region};

use crate::bindings::types::*;
use crate::context::{try_iso_fs, WasiContext};
use crate::errors::StreamError;
use crate::fs_host::Descriptor;
use crate::fs_isolated::{AccessMode, CreateParams, NodeItem};
use crate::stdio::{HostStdin, HostStdout};
use crate::{print_byte_array, EMPTY_BUF};

#[derive(Default)]
pub struct P1Items {
    tree: BTreeMap<u32, P1Item>,
    buf: [u32; 16],
    ix: u8,
}

impl Debug for P1Items {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_map().entries(&self.tree).finish()
    }
}

impl P1Items {
    pub fn new() -> Self {
        Self::default()
    }

    #[instrument(level = Level::DEBUG, skip(self), ret, err(Display))]
    pub fn register(&mut self, item: P1Item) -> AnyResult<Fd> {
        fn f(e: VacantEntry<'_, u32, P1Item>, i: P1Item) -> AnyResult<Fd> {
            let r = *e.key();
            e.insert(i);
            Ok(r.into())
        }

        if self.tree.len() > u32::MAX as usize {
            return Err(crate::errors::FileDescriptorFullError.into());
        }

        while let Some(ix) = self.ix.checked_sub(1) {
            self.ix = ix;
            if let Entry::Vacant(v) = self.tree.entry(self.buf[ix as usize]) {
                return f(v, item);
            }
        }

        let Some((&k, _)) = self.tree.last_key_value() else {
            match self.tree.entry(0) {
                Entry::Vacant(v) => return f(v, item),
                _ => unreachable!("Impossible tree state"),
            }
        };

        if let Some(k) = k.checked_add(1) {
            match self.tree.entry(k) {
                Entry::Vacant(v) => return f(v, item),
                _ => unreachable!("Impossible tree state"),
            }
        }
        for k in (0..u32::MAX).rev() {
            if let Entry::Vacant(v) = self.tree.entry(k) {
                return f(v, item);
            }
        }

        Err(crate::errors::FileDescriptorFullError.into())
    }

    #[instrument(level = Level::DEBUG, skip(self), ret)]
    pub fn unregister(&mut self, fd: Fd) -> Result<P1Item, StreamError> {
        let ix = u32::from(fd);
        let Some(ret) = self.tree.remove(&ix) else {
            warn!(?fd, "Unregister nonexistent descriptor");
            return Err(Errno::Badf.into());
        };
        *if let Some(v) = self.buf.get_mut(self.ix as usize) {
            self.ix += 1;
            v
        } else {
            self.buf.copy_within(1.., 0);
            &mut self.buf[self.buf.len() - 1]
        } = ix;
        Ok(ret)
    }

    pub fn get(&self, fd: Fd) -> Result<&P1Item, StreamError> {
        if let Some(item) = self.tree.get(&fd.into()) {
            debug!(?fd, ?item, "Get descriptor");
            Ok(item)
        } else {
            warn!(?fd, "Get nonexistent descriptor");
            Err(Errno::Badf.into())
        }
    }

    pub fn get_mut(&mut self, fd: Fd) -> Result<&mut P1Item, StreamError> {
        if let Some(item) = self.tree.get_mut(&fd.into()) {
            debug!(?fd, ?item, "Get descriptor");
            Ok(item)
        } else {
            warn!(?fd, "Get nonexistent descriptor");
            Err(Errno::Badf.into())
        }
    }

    #[instrument(level = Level::DEBUG, skip(self))]
    pub fn rename(&mut self, src: Fd, dst: Fd) -> Result<(), StreamError> {
        let v = self.unregister(src)?;
        debug!(fd = ?dst, item = ?v, "Re-register descriptor");
        self.tree.insert(dst.into(), v);
        Ok(())
    }
}

impl FromIterator<P1Item> for P1Items {
    fn from_iter<T>(it: T) -> Self
    where
        T: IntoIterator<Item = P1Item>,
    {
        let mut ret = Self::new();
        for (v, k) in it.into_iter().zip(0u32..) {
            ret.tree.insert(k, v);
        }
        ret
    }
}

#[derive(Debug)]
pub struct P1File {
    preopen: Option<String>,
    cursor: Option<u64>,
    desc: P1Desc,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum P1Desc {
    IsoFS(crate::fs_isolated::CapWrapper),
    HostFS(crate::fs_host::CapWrapper),
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
            cursor: Some(cursor),
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
    (<$l:lifetime, $v:ident> $($i:ident($t:ty, ($t2:ty, $e2:expr), ($t3:ty, $e3:expr))),* $(,)?) => {
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum P1Item {
            $($i($t)),*
        }

        $(
        impl From<$t> for P1Item {
            fn from(v: $t) -> Self {
                Self::$i(v)
            }
        }
        )*

        #[derive(Debug)]
        #[allow(dead_code)]
        enum FdItem<'a> {
            $($i($t2)),*
        }

        #[derive(Debug)]
        #[allow(dead_code)]
        enum FdItemR<'a> {
            $($i($t3)),*
        }

        impl P1Items {
            fn get_item(&mut self, fd: Fd) -> Result<FdItem<'_>, StreamError> {
                Ok(match self.get_mut(fd)? {
                    $(P1Item::$i($v) => FdItem::$i($e2)),*
                })
            }

            fn get_item_ref(&self, fd: Fd) -> Result<FdItemR<'_>, StreamError> {
                Ok(match self.get(fd)? {
                    $(P1Item::$i($v) => FdItemR::$i($e3)),*
                })
            }
        }
    };
}

p1item_gen! {
    <'a, v>
    P1File(Box<P1File>, (&'a mut P1File, v), (&'a P1File, v)),
    StdinSignal(Arc<crate::stdio::StdinSignal>, (&'a Arc<crate::stdio::StdinSignal>, v), (&'a Arc<crate::stdio::StdinSignal>, v)),
    HostStdout(Arc<dyn Send + Sync + HostStdout>, (&'a (dyn Send + Sync + HostStdout), &**v), (&'a (dyn Send + Sync + HostStdout), &**v)),
    HostStdin(Arc<dyn Send + Sync + HostStdin>, (&'a (dyn Send + Sync + HostStdin), &**v), (&'a (dyn Send + Sync + HostStdin), &**v)),
    NullStdio(crate::stdio::NullStdio, (&'a mut crate::stdio::NullStdio, v), (&'a crate::stdio::NullStdio, v)),
}

struct MemIO<'a, 'b, T> {
    mem: &'a mut GuestMemory<'b>,
    len: Size,
    iov: SmallVec<[T; 16]>,
}

impl<'a, 'b> MemIO<'a, 'b, Iovec> {
    fn new_read(mem: &'a mut GuestMemory<'b>, iov: IovecArray) -> Result<Self, StreamError> {
        let iov = iov
            .iter()
            .filter_map(|i| match i.and_then(|p| mem.read(p)) {
                Ok(Iovec { buf_len: 0, .. }) => None,
                v => Some(v),
            })
            .take(256)
            .collect::<Result<SmallVec<[_; 16]>, _>>()?;
        let len = iov.iter().fold(0, |a, v| v.buf_len.saturating_add(a));

        Ok(Self { mem, len, iov })
    }

    #[instrument(level = Level::DEBUG, skip(self, t, f))]
    fn read<T>(
        self,
        mut t: T,
        mut f: impl FnMut(&mut T, Size) -> Result<(Cow<'_, [u8]>, Size), StreamError>,
    ) -> Result<Size, StreamError> {
        let Self { mem, mut len, iov } = self;
        if len == 0 {
            info!("Nothing to read");
            return Ok(0);
        }

        let mut iov = iov.into_iter().inspect(|iov| debug!(?iov, "Iovec"));
        let mut n = 0;
        let Iovec {
            buf: mut p,
            buf_len: mut blen,
        } = iov
            .next()
            .expect("IovecArray ran out before reading complete");
        while len > 0 {
            let (s, mut l) = {
                let _s = debug_span!("Reading from file").entered();
                f(&mut t, len)?
            };
            debug!(length = l, "Read from file");
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
                    } = iov
                        .next()
                        .expect("IovecArray ran out before reading complete");
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

        info!(n, "Total read");
        Ok(n)
    }
}

impl<'a, 'b> MemIO<'a, 'b, Ciovec> {
    fn new_write(mem: &'a mut GuestMemory<'b>, iov: CiovecArray) -> Result<Self, StreamError> {
        let iov = iov
            .iter()
            .filter_map(|i| match i.and_then(|p| mem.read(p)) {
                Ok(Ciovec { buf_len: 0, .. }) => None,
                v => Some(v),
            })
            .take(256)
            .collect::<Result<SmallVec<[_; 16]>, _>>()?;
        let len = iov.iter().fold(0, |a, v| v.buf_len.saturating_add(a));

        Ok(Self { mem, len, iov })
    }

    #[instrument(level = Level::DEBUG, skip(self, f))]
    fn write(
        self,
        mut f: impl FnMut(&[u8]) -> Result<Size, StreamError>,
    ) -> Result<Size, StreamError> {
        let Self { mem, iov, len } = self;
        if len == 0 {
            info!("Nothing to write");
            return Ok(0);
        }

        let Some(Ciovec { buf, buf_len }) = iov
            .into_iter()
            .inspect(|iov| debug!(?iov, "Ciovec"))
            .find(|v| v.buf_len > 0)
        else {
            info!("Nothing to write");
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
        let Some(src) = src else {
            return Err(GuestError::PtrOutOfBounds(Region {
                start: buf,
                len: buf_len,
            })
            .into());
        };

        let ret = {
            let _s = debug_span!("Writing into file").entered();
            f(src)?
        };
        debug!(length = ret, "Written into file");
        Ok(ret)
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
) -> Result<(), StreamError> {
    *dst = match (is_set, is_now) {
        (true, true) => return Err(ErrorKind::InvalidInput.into()),
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
) -> Result<Option<fs_set_times::SystemTimeSpec>, StreamError> {
    match (is_set, is_now) {
        (true, true) => Err(ErrorKind::InvalidInput.into()),
        (true, false) => Ok(Some(fs_set_times::SystemTimeSpec::Absolute(
            SystemTime::UNIX_EPOCH + Duration::from_nanos(time),
        ))),
        (false, true) => Ok(Some(fs_set_times::SystemTimeSpec::SymbolicNow)),
        (false, false) => Ok(None),
    }
}

fn to_utf8_path(s: Cow<'_, str>) -> Cow<'_, Utf8Path> {
    match s {
        Cow::Borrowed(s) => Cow::Borrowed(Utf8Path::new(s)),
        Cow::Owned(s) => Cow::Owned(Utf8PathBuf::from(s)),
    }
}

fn to_path(s: Cow<'_, str>) -> Cow<'_, Path> {
    match s {
        Cow::Borrowed(s) => Cow::Borrowed(Path::new(s)),
        Cow::Owned(s) => Cow::Owned(PathBuf::from(s)),
    }
}

fn from_iso_stamp(stamp: &crate::fs_isolated::Timestamp) -> (Timestamp, Timestamp, Timestamp) {
    (
        to_timestamp(stamp.ctime),
        to_timestamp(stamp.mtime),
        to_timestamp(stamp.atime),
    )
}

fn from_cap_stamp(stamp: std::io::Result<cap_std::time::SystemTime>) -> Timestamp {
    stamp.ok().map_or(0, |v| to_timestamp(v.into_std()))
}

fn iso_filestat(f: &crate::fs_isolated::CapWrapper) -> Filestat {
    let (filetype, size, (ctim, mtim, atim)) = match &f.node().0 {
        NodeItem::Dir(v) => {
            let v = v.lock();
            (Filetype::Directory, v.len(), from_iso_stamp(v.stamp()))
        }
        NodeItem::File(v) => {
            let v = v.lock();
            (Filetype::RegularFile, v.len(), from_iso_stamp(v.stamp()))
        }
        NodeItem::Link(v) => {
            let v = v.read();
            (Filetype::SymbolicLink, v.len(), from_iso_stamp(v.stamp()))
        }
    };

    Filestat {
        dev: 127,
        ino: iso_inode(f.node()),
        nlink: 0,
        size: size as _,
        filetype,
        ctim,
        mtim,
        atim,
    }
}

fn iso_filestat_set_times(
    f: &crate::fs_isolated::CapWrapper,
    atime: Timestamp,
    mtime: Timestamp,
    flags: Fstflags,
) -> Result<(), StreamError> {
    f.access().write_or_err()?;

    f.set_time(|stamp| -> Result<_, StreamError> {
        let now = SystemTime::now();
        set_time(
            &mut stamp.mtime,
            &now,
            mtime,
            flags.contains(Fstflags::MTIM),
            flags.contains(Fstflags::MTIM_NOW),
        )?;
        set_time(
            &mut stamp.atime,
            &now,
            atime,
            flags.contains(Fstflags::ATIM),
            flags.contains(Fstflags::ATIM_NOW),
        )?;
        Ok(())
    })
}

fn host_metadata(f: &crate::fs_host::CapWrapper) -> std::io::Result<cap_std::fs::Metadata> {
    match &**f.desc() {
        Descriptor::File(v) => v.metadata(),
        Descriptor::Dir(v) => v.dir_metadata(),
    }
}

fn host_filestat<H>(m: cap_std::fs::Metadata, hasher: &H) -> Filestat
where
    H: std::hash::BuildHasher,
    H::Hasher: Clone,
{
    let filetype = to_filetype(m.file_type());
    let nlink = m.nlink();
    let size = m.len();
    let ctim = from_cap_stamp(m.created());
    let mtim = from_cap_stamp(m.modified());
    let atim = from_cap_stamp(m.accessed());
    let inode = crate::fs_host::CapWrapper::meta_hash(m, hasher);

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

impl crate::bindings::types::UserErrorConversion for WasiContext {
    #[instrument(level = Level::DEBUG, skip(self), err)]
    fn errno_from_stream_error(&mut self, e: StreamError) -> Result<Errno, AnyError> {
        e.into()
    }
}

impl crate::bindings::wasi_snapshot_preview1::WasiSnapshotPreview1 for WasiContext {
    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn args_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        argv: GuestPtr<GuestPtr<u8>>,
        argv_buf: GuestPtr<u8>,
    ) -> Result<(), StreamError> {
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

    #[instrument(skip(self), err(level = Level::WARN))]
    fn args_sizes_get(&mut self, _: &mut GuestMemory<'_>) -> Result<(Size, Size), StreamError> {
        let cnt = Size::try_from(self.args.len())?;
        let len = self
            .args
            .iter()
            .try_fold(0 as Size, |a, s| {
                a.checked_add(s.len().try_into().ok()?)?.checked_add(1)
            })
            .ok_or(Errno::Overflow)?;
        info!(count = ?cnt, total_length = ?len, "Argument sizes");
        Ok((cnt, len))
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn environ_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        environ: GuestPtr<GuestPtr<u8>>,
        environ_buf: GuestPtr<u8>,
    ) -> Result<(), StreamError> {
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

    #[instrument(skip(self), err(level = Level::WARN))]
    fn environ_sizes_get(&mut self, _: &mut GuestMemory<'_>) -> Result<(Size, Size), StreamError> {
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
        info!(count = ?cnt, total_length = ?len, "Environment sizes");
        Ok((cnt, len))
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn clock_res_get(
        &mut self,
        _: &mut GuestMemory<'_>,
        id: Clockid,
    ) -> Result<Timestamp, StreamError> {
        match id {
            Clockid::Realtime | Clockid::Monotonic => Ok(1000),
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn clock_time_get(
        &mut self,
        _: &mut GuestMemory<'_>,
        id: Clockid,
        _resolution: Timestamp,
    ) -> Result<Timestamp, StreamError> {
        match id {
            Clockid::Realtime => Ok(to_timestamp(SystemTime::now())),
            Clockid::Monotonic => Ok(self.clock.now()),
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_advise(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        off: Filesize,
        len: Filesize,
        advice: Advice,
    ) -> Result<(), StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.file()?.advise(
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
            FdItem::P1File(_)
            | FdItem::StdinSignal(_)
            | FdItem::HostStdin(_)
            | FdItem::HostStdout(_)
            | FdItem::NullStdio(_) => (),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_allocate(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _offset: Filesize,
        _len: Filesize,
    ) -> Result<(), StreamError> {
        self.p1_items.get_item(fd)?;
        Err(ErrorKind::Unsupported.into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_close(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), StreamError> {
        self.p1_items.unregister(fd)?;
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_datasync(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(_),
                ..
            }) => (),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.sync_data()?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_fdstat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Fdstat, StreamError> {
        Ok(match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                cursor,
                ..
            }) => {
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
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                cursor,
                ..
            }) => {
                let f = host_metadata(v)?.file_type();
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
            FdItem::StdinSignal(_) | FdItem::HostStdin(_) => {
                let rights = Rights::FD_READ;
                Fdstat {
                    fs_filetype: Filetype::Unknown,
                    fs_flags: Fdflags::empty(),
                    fs_rights_base: rights,
                    fs_rights_inheriting: rights,
                }
            }
            FdItem::HostStdout(_) => {
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

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_fdstat_set_flags(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _flags: Fdflags,
    ) -> Result<(), StreamError> {
        self.p1_items.get_item(fd)?;
        Err(ErrorKind::Unsupported.into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_fdstat_set_rights(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        _rights_base: Rights,
        _rights_inheriting: Rights,
    ) -> Result<(), StreamError> {
        self.p1_items.get_item(fd)?;
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_filestat_get(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
    ) -> Result<Filestat, StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => Ok(iso_filestat(v)),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => Ok(host_filestat(host_metadata(v)?, &self.hasher)),
            FdItem::StdinSignal(_)
            | FdItem::HostStdin(_)
            | FdItem::HostStdout(_)
            | FdItem::NullStdio(_) => Ok(Filestat {
                dev: 0,
                ino: 0,
                filetype: Filetype::Unknown,
                size: 0,
                nlink: 0,
                ctim: 0,
                mtim: 0,
                atim: 0,
            }),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_filestat_set_size(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        size: Filesize,
    ) -> Result<(), StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => v.resize(size.try_into().map_err(AnyError::from)?)?,
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.write()?.file()?.set_len(size)?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_filestat_set_times(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        atim: Timestamp,
        mtim: Timestamp,
        fst_flags: Fstflags,
    ) -> Result<(), StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => iso_filestat_set_times(v, atim, mtim, fst_flags),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
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
                }
                .map_err(StreamError::from)
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn fd_pread(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: IovecArray,
        offset: Filesize,
    ) -> Result<Size, StreamError> {
        let memio = MemIO::new_read(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                v.access().read_or_err()?;
                let mut off = usize::try_from(offset)?;
                memio.read(v.node().file().ok_or(ErrorKind::IsADirectory)?, |v, len| {
                    let (s, l) = v.read(len.try_into()?, off);
                    off += l;
                    Ok((s.into(), l as Size))
                })
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
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

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_prestat_get(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Prestat, StreamError> {
        if let FdItem::P1File(P1File {
            preopen: Some(s), ..
        }) = self.p1_items.get_item(fd)?
        {
            Ok(Prestat::Dir(PrestatDir {
                pr_name_len: s.len().try_into()?,
            }))
        } else {
            Err(Errno::Badf.into())
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn fd_prestat_dir_name(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<u8>,
        path_len: Size,
    ) -> Result<(), StreamError> {
        if let FdItem::P1File(P1File {
            preopen: Some(s), ..
        }) = self.p1_items.get_item(fd)?
        {
            info!(name = s, "Preopen name");
            if s.len() > usize::try_from(path_len)? {
                return Err(Errno::Nametoolong.into());
            }

            mem.copy_from_slice(s.as_bytes(), path.as_array(s.len().try_into()?))?;
            Ok(())
        } else {
            Err(ErrorKind::NotADirectory.into())
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn fd_pwrite(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: CiovecArray,
        mut offset: Filesize,
    ) -> Result<Size, StreamError> {
        let memio = MemIO::new_write(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                v.access().write_or_err()?;
                let mut v = v.node().file().ok_or(ErrorKind::IsADirectory)?;
                let mut off = usize::try_from(offset)?;
                memio.write(|s| {
                    v.write(s, off)?;
                    off += s.len();
                    Ok(s.len() as Size)
                })
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
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

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn fd_read(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: IovecArray,
    ) -> Result<Size, StreamError> {
        let memio = MemIO::new_read(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                cursor,
                ..
            }) => {
                v.access().read_or_err()?;
                let v = v.node().file().ok_or(ErrorKind::IsADirectory)?;
                let Some(c) = cursor else { return Ok(0) };
                let old = *c;

                let r = memio.read(v, |v, len| {
                    let (s, l) = v.read(
                        len.try_into().unwrap_or(usize::MAX),
                        (*c).try_into().unwrap_or(usize::MAX),
                    );
                    *c += l as u64;
                    Ok((s.into(), l as Size))
                });
                if r.is_err() {
                    *c = old;
                }
                r
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                cursor,
                ..
            }) => {
                // Only try to read once per syscall.
                let mut has_read = false;
                let Some(c) = cursor else { return Ok(0) };
                let old = *c;

                let r = memio.read(v.read()?.file()?, |v, len| {
                    if len == 0 || has_read {
                        return Ok(((&[]).into(), 0));
                    }

                    let mut ret =
                        vec![0; EMPTY_BUF.len().min(len.try_into().unwrap_or(usize::MAX))];
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
            FdItem::NullStdio(_) => memio.read((), |_, len| {
                let l = EMPTY_BUF.len().min(len.try_into().unwrap_or(usize::MAX));
                Ok(((&EMPTY_BUF[..l]).into(), l as Size))
            }),
            FdItem::StdinSignal(v) => memio.read((v, true), |(v, b), len| {
                let len = usize::try_from(len).unwrap_or(usize::MAX);
                let ret = if len > 0 && *b {
                    *b = false;
                    v.read_block(len, self.timeout)
                } else {
                    v.read(len)
                }?;
                let l = ret.len() as Size;
                Ok((ret.into(), l))
            }),
            FdItem::HostStdin(v) => memio.read(v, |v, len| {
                let ret = v.read_block(len.try_into().unwrap_or(usize::MAX), self.timeout)?;
                let l = ret.len() as Size;
                Ok((ret.into(), l))
            }),
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn fd_readdir(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        buf: GuestPtr<u8>,
        buf_len: Size,
        cookie: Dircookie,
    ) -> Result<Size, StreamError> {
        #[instrument(skip(buf, name), fields(name = %print_byte_array(name)))]
        fn write_dirent(buf: &mut [u8], d: Dirent, name: &[u8]) {
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
            buf[Dirent::guest_size() as usize..].copy_from_slice(name);
        }

        fn f<S: AsRef<str>>(
            mem: &mut GuestMemory<'_>,
            buf: GuestPtr<u8>,
            buf_len: Size,
            cookie: Dircookie,
            cur_ino: Inode,
            it: impl IntoIterator<Item = Result<(Inode, Filetype, S), StreamError>>,
        ) -> Result<Size, StreamError> {
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

            let mut i = 0;
            if cookie == 0 {
                let Some((b, r)) = buf.split_at_mut_checked(entry_size + 1) else {
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
                    b".",
                );
                (i, buf) = (i + b.len(), r);
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
                    b"..",
                );
                (i, buf) = (i + b.len(), r);
            }

            for (v, next) in it.into_iter().zip(3 as Dircookie..) {
                let (ino, ft, name) = v?;
                if cookie >= next {
                    continue;
                }

                let name = name.as_ref().as_bytes();
                let nl = usize::try_from(Size::try_from(name.len()).unwrap_or(Size::MAX))?;
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
                    name,
                );
                (i, buf) = (i + b.len(), r);
            }

            Ok(i as _)
        }

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => f(
                mem,
                buf,
                buf_len,
                cookie,
                iso_inode(v.node()),
                v.read_directory()?.map(|v| {
                    v.map(|(k, v)| (v.inode() as Inode, iso_filetype(&v), k))
                        .map_err(StreamError::from)
                }),
            ),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
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
                        let name = v
                            .file_name()
                            .into_string()
                            .ok()
                            .ok_or(ErrorKind::InvalidInput)?;
                        let ft = to_filetype(m.file_type());
                        let inode = crate::fs_host::CapWrapper::meta_hash(m, &self.hasher);
                        Ok((inode.lower ^ inode.upper, ft, name))
                    }),
                )
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_renumber(&mut self, _: &mut GuestMemory<'_>, fd: Fd, to: Fd) -> Result<(), StreamError> {
        self.p1_items.rename(fd, to)
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_seek(
        &mut self,
        _: &mut GuestMemory<'_>,
        fd: Fd,
        offset: Filedelta,
        whence: Whence,
    ) -> Result<Filesize, StreamError> {
        Ok(match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                cursor: Some(c),
                ..
            }) => {
                let l = v.node().file().ok_or(ErrorKind::IsADirectory)?.len() as u64;
                *c = match whence {
                    Whence::Set => offset.try_into().ok(),
                    Whence::Cur => c.checked_add_signed(offset),
                    Whence::End => l.checked_add_signed(offset),
                }
                .ok_or(ErrorKind::InvalidInput)?;
                *c as _
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                cursor: Some(c),
                ..
            }) => {
                let v = v.file()?;
                *c = match whence {
                    Whence::Set => offset.try_into().ok(),
                    Whence::Cur => c.checked_add_signed(offset),
                    Whence::End => v.metadata()?.len().checked_add_signed(offset),
                }
                .ok_or(ErrorKind::InvalidInput)?;
                *c as _
            }
            _ => return Err(Errno::Badf.into()),
        })
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_sync(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<(), StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(_),
                ..
            }) => (),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.sync()?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn fd_tell(&mut self, _: &mut GuestMemory<'_>, fd: Fd) -> Result<Filesize, StreamError> {
        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File { cursor: c, .. }) => Ok(c.unwrap_or(0) as _),
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn fd_write(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        iovs: CiovecArray,
    ) -> Result<Size, StreamError> {
        let memio = MemIO::new_write(mem, iovs)?;

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                cursor,
                ..
            }) => {
                v.access().write_or_err()?;
                let mut v = v.node().file().ok_or(ErrorKind::IsADirectory)?;

                if let Some(c) = cursor {
                    let old = *c;
                    let r = memio.write(|s| {
                        v.write(s, (*c).try_into().unwrap_or(usize::MAX))?;
                        *c += s.len() as u64;
                        Ok(s.len() as Size)
                    });
                    if r.is_err() {
                        *c = old;
                    }
                    r
                } else {
                    memio.write(|s| {
                        let i = v.len();
                        v.write(s, i)?;
                        Ok(s.len() as Size)
                    })
                }
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                cursor,
                ..
            }) => {
                let v = v.write()?.file()?;

                if let Some(c) = cursor {
                    let old = *c;
                    let r = memio.write(|s| {
                        let l = v.write_at(s, *c)?;
                        *c += l as u64;
                        Ok(l as Size)
                    });
                    if r.is_err() {
                        *c = old;
                    }
                    r
                } else {
                    memio.write(|s| {
                        let l = v.append(s)?;
                        Ok(l as Size)
                    })
                }
            }
            FdItem::NullStdio(_) => Ok(memio.len),
            FdItem::HostStdout(v) => {
                let r = memio.write(|s| {
                    v.write(s)?;
                    Ok(s.len() as _)
                })?;
                v.flush()?;
                Ok(r)
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_create_directory(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
    ) -> Result<(), StreamError> {
        let path = mem.as_cow_str(path)?;
        info!(%path, "Arguments");

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                v.access().write_or_err()?;

                let p = to_utf8_path(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let controller = try_iso_fs(&self.iso_fs)?;

                v.open(controller, parent, true, None, AccessMode::W)?
                    .create_dir(controller, name)?;
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.write()?.dir()?.create_dir(to_path(path))?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_filestat_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        flags: Lookupflags,
        path: GuestPtr<str>,
    ) -> Result<Filestat, StreamError> {
        let follow_symlink = flags.contains(Lookupflags::SYMLINK_FOLLOW);
        let path = mem.as_cow_str(path)?;
        info!(%path, follow_symlink, "Arguments");

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => Ok(iso_filestat(&v.open(
                try_iso_fs(&self.iso_fs)?,
                &to_utf8_path(path),
                follow_symlink,
                None,
                AccessMode::R,
            )?)),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
                let v = v.dir()?;
                let p = to_path(path);

                let m = if follow_symlink {
                    v.metadata(p)
                } else {
                    v.symlink_metadata(p)
                }?;
                Ok(host_filestat(m, &self.hasher))
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_filestat_set_times(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        flags: Lookupflags,
        path: GuestPtr<str>,
        atim: Timestamp,
        mtim: Timestamp,
        fst_flags: Fstflags,
    ) -> Result<(), StreamError> {
        let follow_symlink = flags.contains(Lookupflags::SYMLINK_FOLLOW);
        let path = mem.as_cow_str(path)?;
        info!(%path, follow_symlink, "Arguments");

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => iso_filestat_set_times(
                &v.open(
                    try_iso_fs(&self.iso_fs)?,
                    &to_utf8_path(path),
                    follow_symlink,
                    None,
                    AccessMode::W,
                )?,
                atim,
                mtim,
                fst_flags,
            ),
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
                let v = v.dir()?;
                let p = to_path(path);
                let atime = time_cvt(
                    atim,
                    fst_flags.contains(Fstflags::ATIM),
                    fst_flags.contains(Fstflags::ATIM_NOW),
                )?
                .map(cap_fs_ext::SystemTimeSpec::from_std);
                let mtime = time_cvt(
                    mtim,
                    fst_flags.contains(Fstflags::MTIM),
                    fst_flags.contains(Fstflags::MTIM_NOW),
                )?
                .map(cap_fs_ext::SystemTimeSpec::from_std);

                if follow_symlink {
                    DirExt::set_times(v, p, atime, mtime)
                } else {
                    v.set_symlink_times(p, atime, mtime)
                }
                .map_err(StreamError::from)
            }
            _ => Err(Errno::Badf.into()),
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn path_link(
        &mut self,
        _: &mut GuestMemory<'_>,
        src_fd: Fd,
        _src_flags: Lookupflags,
        _src_path: GuestPtr<str>,
        dst_fd: Fd,
        _dst_path: GuestPtr<str>,
    ) -> Result<(), StreamError> {
        self.p1_items.get_item(src_fd)?;
        self.p1_items.get_item(dst_fd)?;
        Err(ErrorKind::Unsupported.into())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_open(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        dirflags: Lookupflags,
        path: GuestPtr<str>,
        oflags: Oflags,
        fs_rights_base: Rights,
        _fs_rights_inheriting: Rights,
        fdflags: Fdflags,
    ) -> Result<Fd, StreamError> {
        let follow_symlink = dirflags.contains(Lookupflags::SYMLINK_FOLLOW);
        let mut access = match (
            fs_rights_base.contains(Rights::FD_READ),
            fs_rights_base.contains(Rights::FD_WRITE),
        ) {
            (_, false) => AccessMode::R,
            (false, true) => AccessMode::W,
            (true, true) => AccessMode::RW,
        };
        let create = oflags.contains(Oflags::CREAT);
        let exclusive = oflags.contains(Oflags::EXCL);
        let is_dir = oflags.contains(Oflags::DIRECTORY);
        let is_truncate = oflags.contains(Oflags::TRUNC);
        let append = fdflags.contains(Fdflags::APPEND);
        let path = mem.as_cow_str(path)?;
        info!(%path, follow_symlink, ?access, create, exclusive, is_dir, is_truncate, append, "Arguments");

        if is_dir && is_truncate {
            return Err(ErrorKind::InvalidInput.into());
        }

        let ret: P1Desc = match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                let create = if create {
                    access = access | AccessMode::W;
                    Some(CreateParams {
                        dir: is_dir,
                        exclusive,
                    })
                } else {
                    None
                };

                let controller = try_iso_fs(&self.iso_fs)?;
                let v = v
                    .open(
                        controller,
                        &to_utf8_path(path),
                        follow_symlink,
                        create,
                        access,
                    )?
                    .follow_symlink(controller)?;
                if is_dir && !v.node().is_dir() {
                    return Err(ErrorKind::NotADirectory.into());
                }
                if is_truncate {
                    v.resize(0)?;
                }

                v.into()
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => {
                let mut opts = cap_std::fs::OpenOptions::new();
                if create {
                    v.access().write_or_err()?;
                    access = access | AccessMode::W;
                    if exclusive {
                        opts.create_new(true);
                    } else {
                        opts.create(true);
                    }
                }
                match access {
                    AccessMode::NA | AccessMode::R => opts.read(true),
                    AccessMode::W => opts.write(true),
                    AccessMode::RW => opts.read(true).write(true),
                };
                if is_truncate {
                    opts.truncate(true);
                }
                opts.follow(if follow_symlink {
                    cap_fs_ext::FollowSymlinks::Yes
                } else {
                    cap_fs_ext::FollowSymlinks::No
                });
                if is_dir {
                    opts.maybe_dir(true);
                }

                let v = v.dir()?.open_with(to_path(path), &opts)?;
                let v = if v.metadata()?.is_dir() {
                    crate::fs_host::Descriptor::Dir(cap_std::fs::Dir::from_std_file(v.into_std()))
                } else if is_dir {
                    return Err(ErrorKind::NotADirectory.into());
                } else {
                    crate::fs_host::Descriptor::File(v)
                };

                crate::fs_host::CapWrapper::new(Arc::new(v), access).into()
            }
            _ => return Err(Errno::Badf.into()),
        };

        let ret = if append {
            P1File::with_append(ret)
        } else {
            P1File::new(ret)
        };
        Ok(self.p1_items.register(Box::new(ret).into())?)
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_readlink(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
        buf: GuestPtr<u8>,
        buf_len: Size,
    ) -> Result<Size, StreamError> {
        let path = mem.as_cow_str(path)?;
        info!(%path, "Arguments");

        let s = match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => v
                .open(
                    try_iso_fs(&self.iso_fs)?,
                    &to_utf8_path(path),
                    false,
                    None,
                    AccessMode::R,
                )?
                .read_link()?,
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v
                .read()?
                .dir()?
                .read_link(to_path(path))?
                .into_os_string()
                .into_string()
                .map_err(|_| ErrorKind::InvalidInput)?,
            _ => return Err(Errno::Badf.into()),
        };
        info!(target = s, "Link read");
        let mut s = s.into_bytes();

        s.truncate(buf_len.try_into().unwrap_or(usize::MAX));
        mem.copy_from_slice(&s, buf.as_array(buf_len))?;
        Ok(s.len() as _)
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_remove_directory(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
    ) -> Result<(), StreamError> {
        let path = mem.as_cow_str(path)?;
        info!(%path, "Arguments");

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                let p = to_utf8_path(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };

                v.open(try_iso_fs(&self.iso_fs)?, parent, true, None, AccessMode::W)?
                    .unlink(name, true)?;
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.write()?.dir()?.remove_dir(to_path(path))?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_rename(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        old_path: GuestPtr<str>,
        new_fd: Fd,
        new_path: GuestPtr<str>,
    ) -> Result<(), StreamError> {
        let src_path = mem.as_cow_str(old_path)?;
        let dst_path = mem.as_cow_str(new_path)?;
        info!(%src_path, %dst_path, "Arguments");

        match (
            self.p1_items.get_item_ref(fd)?,
            self.p1_items.get_item_ref(new_fd)?,
        ) {
            (
                FdItemR::P1File(P1File {
                    desc: P1Desc::IsoFS(src),
                    ..
                }),
                FdItemR::P1File(P1File {
                    desc: P1Desc::IsoFS(dst),
                    ..
                }),
            ) => {
                let (src_path, dst_path) = (to_utf8_path(src_path), to_utf8_path(dst_path));
                let (src_path, Some(src_file), dst_path, Some(dst_file)) = (
                    src_path.parent().unwrap_or(&src_path),
                    src_path.file_name(),
                    dst_path.parent().unwrap_or(&dst_path),
                    dst_path.file_name(),
                ) else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let controller = try_iso_fs(&self.iso_fs)?;

                let src = src.open(controller, src_path, true, None, AccessMode::W)?;
                let dst = dst.open(controller, dst_path, true, None, AccessMode::W)?;

                dst.move_file(src.node(), src_file, dst_file)?;
            }
            (
                FdItemR::P1File(P1File {
                    desc: P1Desc::HostFS(src),
                    ..
                }),
                FdItemR::P1File(P1File {
                    desc: P1Desc::HostFS(dst),
                    ..
                }),
            ) => src.write()?.dir()?.rename(
                to_path(src_path),
                dst.write()?.dir()?,
                to_path(dst_path),
            )?,
            (FdItemR::P1File(_), FdItemR::P1File(_)) => return Err(Errno::Xdev.into()),
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_symlink(
        &mut self,
        mem: &mut GuestMemory<'_>,
        old_path: GuestPtr<str>,
        fd: Fd,
        new_path: GuestPtr<str>,
    ) -> Result<(), StreamError> {
        let path = mem.as_cow_str(old_path)?;
        let target = mem.as_cow_str(new_path)?;
        info!(%path, %target, "Arguments");

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                let p = to_utf8_path(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let controller = try_iso_fs(&self.iso_fs)?;

                v.open(controller, parent, true, None, AccessMode::W)?
                    .create_link(controller, name, &to_utf8_path(target))?;
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(_),
                ..
            }) => return Err(ErrorKind::Unsupported.into()),
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn path_unlink_file(
        &mut self,
        mem: &mut GuestMemory<'_>,
        fd: Fd,
        path: GuestPtr<str>,
    ) -> Result<(), StreamError> {
        let path = mem.as_cow_str(path)?;
        info!(%path, "Arguments");

        match self.p1_items.get_item(fd)? {
            FdItem::P1File(P1File {
                desc: P1Desc::IsoFS(v),
                ..
            }) => {
                let p = to_utf8_path(path);
                let (parent, Some(name)) = (p.parent().unwrap_or(&p), p.file_name()) else {
                    return Err(ErrorKind::InvalidInput.into());
                };

                v.open(try_iso_fs(&self.iso_fs)?, parent, true, None, AccessMode::W)?
                    .unlink(name, false)?;
            }
            FdItem::P1File(P1File {
                desc: P1Desc::HostFS(v),
                ..
            }) => v.write()?.dir()?.remove_file_or_symlink(to_path(path))?,
            _ => return Err(Errno::Badf.into()),
        }
        Ok(())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn poll_oneoff(
        &mut self,
        mem: &mut GuestMemory<'_>,
        in_: GuestPtr<Subscription>,
        out: GuestPtr<Event>,
        nsubscriptions: Size,
    ) -> Result<Size, StreamError> {
        if nsubscriptions == 0 {
            return Err(ErrorKind::InvalidInput.into());
        }

        enum Poll {
            Always,
            Instant(Instant),
            SystemTime(SystemTime),
            Signal(crate::stdio::StdinSignalPollable),
        }

        let now = Instant::now();
        let polls = in_
            .as_array(nsubscriptions)
            .iter()
            .map(|i| {
                let sub = mem.read(i?)?;
                Ok((
                    match &sub.u {
                        SubscriptionU::Clock(v) => match v.id {
                            Clockid::Monotonic | Clockid::Realtime
                                if !v.flags.contains(Subclockflags::SUBSCRIPTION_CLOCK_ABSTIME) =>
                            {
                                Poll::Instant(now + Duration::from_nanos(v.timeout))
                            }
                            Clockid::Monotonic => {
                                Poll::Instant(self.clock.poll_until(v.timeout)?.until)
                            }
                            Clockid::Realtime => Poll::SystemTime(
                                SystemTime::UNIX_EPOCH + Duration::from_nanos(v.timeout),
                            ),
                            _ => return Err(ErrorKind::InvalidInput.into()),
                        },
                        SubscriptionU::FdRead(v) => {
                            match self.p1_items.get_item(v.file_descriptor)? {
                                // File is always ready
                                FdItem::P1File(_) | FdItem::NullStdio(_) | FdItem::HostStdin(_) => {
                                    Poll::Always
                                }
                                FdItem::StdinSignal(v) => Poll::Signal(v.poll()?),
                                _ => return Err(Errno::Badf.into()),
                            }
                        }
                        SubscriptionU::FdWrite(v) => {
                            match self.p1_items.get_item(v.file_descriptor)? {
                                // File is always ready
                                FdItem::P1File(_)
                                | FdItem::NullStdio(_)
                                | FdItem::HostStdout(_) => Poll::Always,
                                _ => return Err(Errno::Badf.into()),
                            }
                        }
                    },
                    sub,
                ))
            })
            .collect::<Result<Vec<_>, StreamError>>()?;

        let mut controller: Option<crate::poll::PollController> = None;
        for _ in 0..3 {
            let mut n: Size = 0;
            let now = Instant::now();
            let now_st = SystemTime::now();
            for (p, s) in &polls {
                if !match p {
                    Poll::Always => true,
                    Poll::Instant(t) => *t <= now,
                    Poll::SystemTime(t) => *t <= now_st,
                    Poll::Signal(v) => {
                        controller.as_ref().is_some_and(|c| c.is_waited(&v.0)) || v.is_ready()
                    }
                } {
                    continue;
                }

                mem.write(
                    out.add(n)?,
                    Event {
                        userdata: s.userdata,
                        error: Errno::Success,
                        type_: match s.u {
                            SubscriptionU::Clock(_) => Eventtype::Clock,
                            SubscriptionU::FdRead(_) => Eventtype::FdRead,
                            SubscriptionU::FdWrite(_) => Eventtype::FdWrite,
                        },
                        fd_readwrite: match s.u {
                            SubscriptionU::Clock(_) => EventFdReadwrite {
                                flags: Eventrwflags::empty(),
                                nbytes: 0,
                            },
                            SubscriptionU::FdRead(_) | SubscriptionU::FdWrite(_) => {
                                EventFdReadwrite {
                                    flags: Eventrwflags::empty(),
                                    nbytes: EMPTY_BUF.len() as _,
                                }
                            }
                        },
                    },
                )?;
                n += 1;
            }

            if n > 0 {
                return Ok(n);
            }

            let c = controller.get_or_insert_with(|| {
                let mut c = crate::poll::PollController::new(self.timeout);
                for (p, _) in &polls {
                    match p {
                        Poll::Always => (),
                        Poll::Instant(t) => c.set_instant(*t),
                        Poll::SystemTime(t) => c.set_systime(*t),
                        Poll::Signal(v) => c.add_signal(&v.0),
                    }
                }

                c
            });
            if c.poll() {
                break;
            }
        }

        Ok(0)
    }

    #[instrument(skip(self), ret)]
    fn proc_exit(&mut self, _: &mut GuestMemory<'_>, rval: Exitcode) -> AnyError {
        crate::errors::ProcessExit::new(rval).into()
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn proc_raise(&mut self, _: &mut GuestMemory<'_>, _signal: Signal) -> Result<(), StreamError> {
        Err(ErrorKind::Unsupported.into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sched_yield(&mut self, _: &mut GuestMemory<'_>) -> Result<(), StreamError> {
        Ok(())
    }

    #[instrument(skip(self, mem), err(level = Level::WARN))]
    fn random_get(
        &mut self,
        mem: &mut GuestMemory<'_>,
        buf: GuestPtr<u8>,
        buf_len: Size,
    ) -> Result<(), StreamError> {
        let buf = buf.offset();
        let s = usize::try_from(buf)?;
        let l = usize::try_from(buf_len)?;

        let src = match mem {
            GuestMemory::Unshared(mem) => mem.get_mut(s..).and_then(|v| v.get_mut(..l)),
            GuestMemory::Shared(mem) => mem.get(s..).and_then(|v| v.get(..l)).map(|v| {
                // SAFETY: Responsibility for shared access exclusivity is on guest.
                #[allow(mutable_transmutes)]
                unsafe {
                    transmute::<&[UnsafeCell<u8>], &mut [u8]>(v)
                }
            }),
        };
        if let Some(src) = src {
            self.secure_rng.fill(src);
            Ok(())
        } else {
            Err(GuestError::PtrOutOfBounds(Region {
                start: buf,
                len: buf_len,
            })
            .into())
        }
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sock_accept(
        &mut self,
        _: &mut GuestMemory<'_>,
        _fd: Fd,
        _flags: Fdflags,
    ) -> Result<Fd, StreamError> {
        Err(Errno::Notsock.into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sock_recv(
        &mut self,
        _: &mut GuestMemory<'_>,
        _fd: Fd,
        _iov: IovecArray,
        _flags: Riflags,
    ) -> Result<(Size, Roflags), StreamError> {
        Err(Errno::Notsock.into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sock_send(
        &mut self,
        _: &mut GuestMemory<'_>,
        _fd: Fd,
        _iov: CiovecArray,
        _flags: Siflags,
    ) -> Result<Size, StreamError> {
        Err(Errno::Notsock.into())
    }

    #[instrument(skip(self), err(level = Level::WARN))]
    fn sock_shutdown(
        &mut self,
        _: &mut GuestMemory<'_>,
        _fd: Fd,
        _flags: Sdflags,
    ) -> Result<(), StreamError> {
        Err(Errno::Notsock.into())
    }
}
