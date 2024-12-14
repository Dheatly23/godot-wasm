use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::io::{Error as IoError, ErrorKind};

use anyhow::Error as AnyError;

use crate::bindings::wasi::filesystem::types::ErrorCode;
use crate::bindings::wasi::io::streams::StreamError as WasiStreamError;

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

pub(crate) struct WasiFSError(ErrorCode);

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

pub struct StreamError(StreamErrorInner);

enum StreamErrorInner {
    Any(AnyError),
    Io(IoError),
    Wasi(ErrorCode),
    Closed,
}

impl StreamError {
    pub const fn closed() -> Self {
        Self(StreamErrorInner::Closed)
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

impl From<ErrorCode> for StreamError {
    fn from(v: ErrorCode) -> Self {
        Self(StreamErrorInner::Wasi(v))
    }
}

impl From<StreamError> for Result<ErrorCode, AnyError> {
    fn from(v: StreamError) -> Self {
        Ok(match v.0 {
            StreamErrorInner::Any(v) => return Err(v),
            StreamErrorInner::Closed => return Err(StreamClosedError.into()),
            StreamErrorInner::Wasi(v) => v,
            StreamErrorInner::Io(v) => match v.kind() {
                ErrorKind::Other => return Err(v.into()),
                ErrorKind::NotFound => ErrorCode::NoEntry,
                ErrorKind::PermissionDenied => ErrorCode::NotPermitted,
                ErrorKind::AlreadyExists => ErrorCode::Exist,
                ErrorKind::InvalidInput => ErrorCode::Invalid,
                ErrorKind::Deadlock => ErrorCode::Deadlock,
                ErrorKind::DirectoryNotEmpty => ErrorCode::NotEmpty,
                ErrorKind::IsADirectory => ErrorCode::IsDirectory,
                ErrorKind::NotADirectory => ErrorCode::NotDirectory,
                ErrorKind::ReadOnlyFilesystem => ErrorCode::ReadOnly,
                ErrorKind::NotSeekable => ErrorCode::InvalidSeek,
                ErrorKind::Unsupported => ErrorCode::Unsupported,
                _ => ErrorCode::Io,
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
