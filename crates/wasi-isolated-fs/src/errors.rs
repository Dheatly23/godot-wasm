use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result};

pub(crate) enum NodeItemTy {
    Dir,
    File,
    Link,
}

impl Display for NodeItemTy {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(self, f)
    }
}

impl Display for WrongNodeItemError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(self, f)
    }
}

impl Display for FileLimitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Size(v) => write!(f, "trying to acquire {v} bytes, but file limit reached"),
            Self::Node => write!(f, "trying to allocate node, but file limit reached"),
        }
    }
}

impl Error for FileLimitError {}

pub(crate) struct AccessError;

impl Debug for AccessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(self, f)
    }
}

impl Display for AccessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "access control error")
    }
}

impl Error for AccessError {}
