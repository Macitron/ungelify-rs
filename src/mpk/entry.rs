use crate::mpk::bytes;
use crate::mpk::bytes::{MpkEntryV1, MpkEntryV2};
use flate2::read::ZlibDecoder;
use std::io;
use std::io::{Read, Write};

#[derive(Debug)]
pub struct MagesEntry {
    id: u32,
    name: String,
    offset: u64,
    len_deflated: u64,
    len_compressed: u64,
    is_compressed: bool,
}

impl MagesEntry {
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    #[allow(clippy::missing_const_for_fn)] // compilation error if it's made const
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn len_deflated(&self) -> u64 {
        self.len_deflated
    }

    #[must_use]
    pub const fn len_compressed(&self) -> u64 {
        self.len_compressed
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub fn extract<R: Read, W: Write>(&self, reader: &mut R, writer: &mut W) {
        let mut reader = reader.take(self.len_compressed);
        if self.is_compressed {
            let mut zlib_reader = ZlibDecoder::new(reader);
            io::copy(&mut zlib_reader, writer).expect("failed to copy entry from zlib reader");
        } else {
            io::copy(&mut reader, writer).expect("failed to copy entry from reader");
        }
    }
}

impl From<MpkEntryV1> for MagesEntry {
    fn from(entry: MpkEntryV1) -> Self {
        Self {
            id: entry.id,
            name: bytes::entry_name_from_bytes(&entry.name),
            offset: u64::from(entry.offset),
            len_deflated: u64::from(entry.len_deflated),
            len_compressed: u64::from(entry.len_compressed),
            is_compressed: false,
        }
    }
}

impl From<MpkEntryV2> for MagesEntry {
    fn from(entry: MpkEntryV2) -> Self {
        Self {
            id: entry.id,
            name: bytes::entry_name_from_bytes(&entry.name),
            offset: entry.offset,
            len_deflated: entry.len_deflated,
            len_compressed: entry.len_compressed,
            is_compressed: entry.len_compressed != entry.len_deflated,
        }
    }
}
