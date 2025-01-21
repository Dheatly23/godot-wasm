use std::io::{Read, Result as IoResult};

use godot::prelude::*;

use crate::godot_util::SendSyncWrapper;

pub struct PackedByteArrayReader {
    data: SendSyncWrapper<PackedByteArray>,
    cursor: usize,
}

impl From<PackedByteArray> for PackedByteArrayReader {
    fn from(v: PackedByteArray) -> Self {
        Self {
            data: SendSyncWrapper::new(v),
            cursor: 0,
        }
    }
}

impl Read for PackedByteArrayReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let s = self.data.as_slice();
        let Some(s) = s.get(self.cursor..) else {
            return Ok(0);
        };
        let l = s.len().min(buf.len());
        buf[..l].copy_from_slice(&s[..l]);
        self.cursor += l;
        Ok(l)
    }
}
