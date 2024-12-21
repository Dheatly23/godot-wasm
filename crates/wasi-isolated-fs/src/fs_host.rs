use std::hash::{BuildHasher, Hasher};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result as AnyResult;
use cap_fs_ext::MetadataExt;
use cap_std::fs::{Dir as CapDir, DirEntry, File as CapFile, Metadata, ReadDir as CapReadDir};
use parking_lot::Mutex;
use system_interface::fs::FileIoExt;

use crate::bindings::wasi;
use crate::errors;
use crate::fs_isolated::AccessMode;
#[doc(no_inline)]
pub use crate::fs_isolated::OpenMode;

pub enum Descriptor {
    Dir(CapDir),
    File(CapFile),
}

impl Descriptor {
    pub fn try_file(&self) -> AnyResult<&CapFile> {
        match self {
            Self::File(v) => Ok(v),
            _ => Err(errors::WrongNodeItemError {
                exp: errors::NodeItemTy::File,
                ty: errors::NodeItemTy::Dir,
            }
            .into()),
        }
    }

    pub fn try_dir(&self) -> AnyResult<&CapDir> {
        match self {
            Self::Dir(v) => Ok(v),
            _ => Err(errors::WrongNodeItemError {
                exp: errors::NodeItemTy::Dir,
                ty: errors::NodeItemTy::File,
            }
            .into()),
        }
    }

    pub fn file(&self) -> Option<&CapFile> {
        match self {
            Self::File(v) => Some(v),
            _ => None,
        }
    }

    pub fn dir(&self) -> Option<&CapDir> {
        match self {
            Self::Dir(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct CapWrapper {
    desc: Arc<Descriptor>,
    access: AccessMode,
}

impl CapWrapper {
    #[inline(always)]
    pub fn new(desc: Arc<Descriptor>, access: AccessMode) -> Self {
        Self { desc, access }
    }

    #[inline(always)]
    pub fn desc(&self) -> &Arc<Descriptor> {
        &self.desc
    }

    #[inline(always)]
    pub fn access(&self) -> AccessMode {
        self.access
    }

    pub(crate) fn read(&self) -> Result<&Self, errors::StreamError> {
        self.access.read_or_err()?;
        Ok(self)
    }

    pub(crate) fn write(&self) -> Result<&Self, errors::StreamError> {
        self.access.write_or_err()?;
        Ok(self)
    }

    pub(crate) fn meta_hash<H>(
        m: Metadata,
        hasher: &H,
    ) -> wasi::filesystem::types::MetadataHashValue
    where
        H: BuildHasher,
        H::Hasher: Clone,
    {
        let mut h1 = hasher.build_hasher();
        h1.write_u64(m.dev());
        h1.write_u64(m.ino());

        let mut h2 = h1.clone();
        h1.write_u32(0xc12af7ed);
        h2.write_u128(0x00265409_00274028_00288693);

        wasi::filesystem::types::MetadataHashValue {
            lower: h1.finish(),
            upper: h2.finish(),
        }
    }

    pub fn metadata_hash<H>(
        &self,
        hasher: &H,
    ) -> Result<wasi::filesystem::types::MetadataHashValue, errors::StreamError>
    where
        H: BuildHasher,
        H::Hasher: Clone,
    {
        Ok(Self::meta_hash(
            match &*self.desc {
                Descriptor::File(v) => v.metadata(),
                Descriptor::Dir(v) => v.dir_metadata(),
            }?,
            hasher,
        ))
    }

    pub fn metadata_hash_at<H>(
        &self,
        path: impl AsRef<Path>,
        follow_symlink: bool,
        hasher: &H,
    ) -> Result<wasi::filesystem::types::MetadataHashValue, errors::StreamError>
    where
        H: BuildHasher,
        H::Hasher: Clone,
    {
        let v = self.dir()?;
        Ok(Self::meta_hash(
            if follow_symlink {
                v.metadata(path)
            } else {
                v.symlink_metadata(path)
            }?,
            hasher,
        ))
    }

    pub fn open_file(&self, mode: OpenMode) -> Result<FileStream, errors::StreamError> {
        if let OpenMode::Read(_) = mode {
            self.access.read_or_err()?
        } else {
            self.access.write_or_err()?;
        }

        match *self.desc {
            Descriptor::File(_) => Ok(FileStream {
                file: self.desc.clone(),
                mode,
            }),
            _ => Err(ErrorKind::IsADirectory.into()),
        }
    }

    pub fn read_dir(&self) -> Result<ReadDir, errors::StreamError> {
        self.access.read_or_err()?;
        Ok(ReadDir(Mutex::new(self.desc.try_dir()?.entries()?)))
    }

    pub fn file(&self) -> Result<&CapFile, errors::StreamError> {
        match &*self.desc {
            Descriptor::File(v) => Ok(v),
            _ => Err(ErrorKind::IsADirectory.into()),
        }
    }

    pub fn dir(&self) -> Result<&CapDir, errors::StreamError> {
        match &*self.desc {
            Descriptor::Dir(v) => Ok(v),
            _ => Err(ErrorKind::NotADirectory.into()),
        }
    }
}

pub struct FileStream {
    file: Arc<Descriptor>,
    mode: OpenMode,
}

impl FileStream {
    pub fn read(&mut self, len: usize) -> Result<Vec<u8>, errors::StreamError> {
        let OpenMode::Read(cursor) = &mut self.mode else {
            return Err(ErrorKind::PermissionDenied.into());
        };
        let file = self.file.try_file()?;

        let mut ret = vec![0; len];
        let mut i = 0;
        while i < ret.len() {
            let l = file.read_at(&mut ret[i..], *cursor as _)?;
            if l == 0 {
                break;
            }
            i += l;
            *cursor += l;
        }
        ret.truncate(i);
        Ok(ret)
    }

    pub fn skip(&mut self, len: usize) -> Result<(), errors::StreamError> {
        let OpenMode::Read(cursor) = &mut self.mode else {
            return Err(ErrorKind::PermissionDenied.into());
        };
        *cursor += len;
        Ok(())
    }

    pub fn write(&mut self, mut buf: &[u8]) -> Result<(), errors::StreamError> {
        let file = self.file.try_file()?;

        match &mut self.mode {
            OpenMode::Read(_) => return Err(ErrorKind::PermissionDenied.into()),
            OpenMode::Write(cursor) => {
                while !buf.is_empty() {
                    let l = file.write_at(buf, *cursor as _)?;
                    buf = &buf[l..];
                    *cursor += l;
                }
            }
            OpenMode::Append => {
                while !buf.is_empty() {
                    let l = file.append(buf)?;
                    buf = &buf[l..];
                }
            }
        }
        Ok(())
    }
}

pub struct ReadDir(Mutex<CapReadDir>);

impl ReadDir {
    pub fn next(&self) -> Option<Result<DirEntry, errors::StreamError>> {
        let ret = self.0.lock().next();
        match ret {
            None => None,
            Some(Ok(v)) => Some(Ok(v)),
            Some(Err(e)) => Some(Err(e.into())),
        }
    }
}
