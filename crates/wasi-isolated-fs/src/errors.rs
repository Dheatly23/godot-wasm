use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::io::{Error as IoError, ErrorKind};
use std::num::TryFromIntError;

use anyhow::Error as AnyError;

use crate::bindings::wasi::filesystem::types::ErrorCode as FSErrorCode;
use crate::bindings::wasi::io::streams::StreamError as WasiStreamError;
use crate::bindings::wasi::sockets::network::ErrorCode as NetErrorCode;

pub(crate) enum NodeItemTy {
    Dir,
    File,
    Link,
}

impl Display for NodeItemTy {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
        write!(
            fmt,
            "{}",
            match self {
                Self::Dir => "directory",
                Self::File => "file",
                Self::Link => "symbolic link",
            }
        )
    }
}

pub(crate) struct WrongNodeItemError {
    pub(crate) exp: NodeItemTy,
    pub(crate) ty: NodeItemTy,
}

impl Debug for WrongNodeItemError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for WrongNodeItemError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "node type mismatch! (expected {}, got {})",
            self.exp, self.ty
        )
    }
}

impl Error for WrongNodeItemError {}

pub(crate) struct BuilderIsoFSDefinedError;

impl Debug for BuilderIsoFSDefinedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for BuilderIsoFSDefinedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "isolated filesystem is already set")
    }
}

impl Error for BuilderIsoFSDefinedError {}

pub(crate) struct BuilderIsoFSNotDefinedError;

impl Debug for BuilderIsoFSNotDefinedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for BuilderIsoFSNotDefinedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "isolated filesystem is not set")
    }
}

impl Error for BuilderIsoFSNotDefinedError {}

pub(crate) struct BuilderStdioDefinedError;

impl Debug for BuilderStdioDefinedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for BuilderStdioDefinedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "stdio is already set")
    }
}

impl Error for BuilderStdioDefinedError {}

pub(crate) struct RelativePathError;

impl Debug for RelativePathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for RelativePathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "current working directory path should be absolute")
    }
}

impl Error for RelativePathError {}

pub(crate) enum FileLimitError {
    Size(usize),
    Node,
}

impl Debug for FileLimitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for FileLimitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Size(v) => write!(f, "trying to acquire {v} bytes, but file limit reached"),
            Self::Node => write!(f, "trying to allocate node, but file limit reached"),
        }
    }
}

impl Error for FileLimitError {}

pub(crate) struct InvalidPathError(pub(crate) String);

impl Debug for InvalidPathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for InvalidPathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "path {:?} is invalid", self.0)
    }
}

impl Error for InvalidPathError {}

pub(crate) struct PathAlreadyExistError(pub(crate) String);

impl Debug for PathAlreadyExistError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for PathAlreadyExistError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "path {:?} already exist", self.0)
    }
}

impl Error for PathAlreadyExistError {}

#[derive(Default)]
pub(crate) struct InvalidResourceIDError {
    ids: [u32; 32],
    n: u8,
}

impl Debug for InvalidResourceIDError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for InvalidResourceIDError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.n {
            0 => return write!(f, "resource ID not found"),
            1 => return write!(f, "resource ID not found: {}", self.ids[0]),
            _ => (),
        }

        write!(f, "resource IDs not found: ")?;
        for (i, v) in self.ids[..self.ids.len().min(self.n as usize)]
            .iter()
            .enumerate()
        {
            write!(f, "{}{}", if i == 0 { "" } else { ", " }, v)?;
        }
        if self.n as usize > self.ids.len() {
            write!(f, ", ...")?;
        }

        Ok(())
    }
}

impl Error for InvalidResourceIDError {}

impl FromIterator<u32> for InvalidResourceIDError {
    fn from_iter<T: IntoIterator<Item = u32>>(it: T) -> Self {
        let mut ret = Self::default();
        ret.extend(it);
        ret
    }
}

impl Extend<u32> for InvalidResourceIDError {
    fn extend<T: IntoIterator<Item = u32>>(&mut self, it: T) {
        if self.n as usize > self.ids.len() {
            return;
        }
        for id in it {
            match self.ids.len().cmp(&(self.n as usize)) {
                Ordering::Less => return,
                Ordering::Equal => {
                    if self.ids.binary_search(&id).is_err() {
                        self.n += 1;
                    }
                    return;
                }
                Ordering::Greater => (),
            }
            let Err(i) = self.ids[..self.n as usize].binary_search(&id) else {
                continue;
            };
            self.ids.copy_within(i..self.n as usize, i + 1);
            self.ids[i] = id;
            self.n += 1;
        }
    }
}

impl InvalidResourceIDError {
    pub(crate) fn is_empty(&self) -> bool {
        self.n == 0
    }
}

pub(crate) struct StreamClosedError;

impl Debug for StreamClosedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for StreamClosedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "stream unexpectedly closed")
    }
}

impl Error for StreamClosedError {}

pub(crate) struct MonotonicClockError;

impl Debug for MonotonicClockError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for MonotonicClockError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "unknown monotonic clock error")
    }
}

impl Error for MonotonicClockError {}

pub(crate) struct NetworkUnsupportedError;

impl Debug for NetworkUnsupportedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for NetworkUnsupportedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "network access denied")
    }
}

impl Error for NetworkUnsupportedError {}

pub(crate) struct WasiFSError(FSErrorCode);

impl Debug for WasiFSError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for WasiFSError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "WASI file error: {}", self.0)
    }
}

impl Error for WasiFSError {}

pub(crate) struct WasiNetError(NetErrorCode);

impl Debug for WasiNetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for WasiNetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "WASI network error: {}", self.0)
    }
}

impl Error for WasiNetError {}

pub struct StreamError(StreamErrorInner);

enum StreamErrorInner {
    Any(AnyError),
    Io(IoError),
    Wasi(FSErrorCode),
    Closed,
}

impl StreamError {
    pub const fn closed() -> Self {
        Self(StreamErrorInner::Closed)
    }
}

impl From<TryFromIntError> for StreamError {
    fn from(_: TryFromIntError) -> Self {
        Self(StreamErrorInner::Wasi(FSErrorCode::Overflow))
    }
}

impl From<AnyError> for StreamError {
    fn from(v: AnyError) -> Self {
        Self(StreamErrorInner::Any(v))
    }
}

impl From<IoError> for StreamError {
    fn from(v: IoError) -> Self {
        Self(StreamErrorInner::Io(v))
    }
}

impl From<ErrorKind> for StreamError {
    fn from(v: ErrorKind) -> Self {
        Self(StreamErrorInner::Io(v.into()))
    }
}

impl From<FSErrorCode> for StreamError {
    fn from(v: FSErrorCode) -> Self {
        Self(StreamErrorInner::Wasi(v))
    }
}

impl From<StreamError> for Result<FSErrorCode, AnyError> {
    fn from(v: StreamError) -> Self {
        Ok(match v.0 {
            StreamErrorInner::Any(v) => return Err(v),
            StreamErrorInner::Closed => return Err(StreamClosedError.into()),
            StreamErrorInner::Wasi(v) => v,
            StreamErrorInner::Io(v) => match v.kind() {
                ErrorKind::Other => return Err(v.into()),
                ErrorKind::NotFound => FSErrorCode::NoEntry,
                ErrorKind::PermissionDenied => FSErrorCode::NotPermitted,
                ErrorKind::AlreadyExists => FSErrorCode::Exist,
                ErrorKind::InvalidInput => FSErrorCode::Invalid,
                ErrorKind::Deadlock => FSErrorCode::Deadlock,
                ErrorKind::DirectoryNotEmpty => FSErrorCode::NotEmpty,
                ErrorKind::IsADirectory => FSErrorCode::IsDirectory,
                ErrorKind::NotADirectory => FSErrorCode::NotDirectory,
                ErrorKind::ReadOnlyFilesystem => FSErrorCode::ReadOnly,
                ErrorKind::NotSeekable => FSErrorCode::InvalidSeek,
                ErrorKind::Unsupported => FSErrorCode::Unsupported,
                ErrorKind::InvalidData => FSErrorCode::IllegalByteSequence,
                _ => FSErrorCode::Io,
            },
        })
    }
}

impl From<StreamError> for Result<WasiStreamError, AnyError> {
    fn from(v: StreamError) -> Self {
        match v.0 {
            StreamErrorInner::Any(v) => Err(v),
            StreamErrorInner::Io(v) => Err(v.into()),
            StreamErrorInner::Wasi(v) => Err(WasiFSError(v).into()),
            StreamErrorInner::Closed => Ok(WasiStreamError::Closed),
        }
    }
}

impl From<StreamError> for AnyError {
    fn from(v: StreamError) -> Self {
        match v.0 {
            StreamErrorInner::Any(v) => v,
            StreamErrorInner::Io(v) => v.into(),
            StreamErrorInner::Wasi(v) => WasiFSError(v).into(),
            StreamErrorInner::Closed => StreamClosedError.into(),
        }
    }
}

impl From<StreamError> for crate::bindings::types::Error {
    fn from(v: StreamError) -> Self {
        match <Result<FSErrorCode, AnyError>>::from(v) {
            Ok(v) => crate::bindings::types::Errno::from(v).into(),
            Err(e) => Self::trap(e),
        }
    }
}

impl StreamError {
    #[inline(always)]
    pub fn io(&self) -> Option<&IoError> {
        match &self.0 {
            StreamErrorInner::Io(v) => Some(v),
            _ => None,
        }
    }
}

pub struct NetworkError(NetworkErrorInner);

enum NetworkErrorInner {
    Any(AnyError),
    Io(IoError),
    Wasi(NetErrorCode),
}

impl From<AnyError> for NetworkError {
    fn from(v: AnyError) -> Self {
        Self(NetworkErrorInner::Any(v))
    }
}

impl From<IoError> for NetworkError {
    fn from(v: IoError) -> Self {
        Self(NetworkErrorInner::Io(v))
    }
}

impl From<ErrorKind> for NetworkError {
    fn from(v: ErrorKind) -> Self {
        Self(NetworkErrorInner::Io(v.into()))
    }
}

impl From<NetErrorCode> for NetworkError {
    fn from(v: NetErrorCode) -> Self {
        Self(NetworkErrorInner::Wasi(v))
    }
}

impl From<NetworkError> for Result<NetErrorCode, AnyError> {
    fn from(v: NetworkError) -> Self {
        Ok(match v.0 {
            NetworkErrorInner::Any(v) => return Err(v),
            NetworkErrorInner::Wasi(v) => v,
            // For now no mapping
            NetworkErrorInner::Io(v) => return Err(v.into()),
        })
    }
}

impl From<NetworkError> for AnyError {
    fn from(v: NetworkError) -> Self {
        match v.0 {
            NetworkErrorInner::Any(v) => v,
            NetworkErrorInner::Io(v) => v.into(),
            NetworkErrorInner::Wasi(v) => WasiNetError(v).into(),
        }
    }
}

#[derive(Default)]
pub struct ProcessExit {
    pub code: u32,
}

impl Debug for ProcessExit {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl Display for ProcessExit {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.is_success() {
            write!(f, "WASM code successfully exited")
        } else {
            write!(f, "WASM code exited with code {}", self.code)
        }
    }
}

impl Error for ProcessExit {}

impl ProcessExit {
    pub const fn new(code: u32) -> Self {
        Self { code }
    }

    pub const fn is_success(&self) -> bool {
        self.code == 0
    }
}
