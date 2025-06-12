use crate::mpk::bytes;
use crate::mpk::bytes::{MpkEntryV1, MpkEntryV2};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io;
use std::io::{Read, Write};

#[derive(Debug)]
pub struct MagesEntry {
    id: u32,
    name: String,
    offset: u64,
    len_deflated: u64,
    len_compressed: u64,
    pub(super) cpr_indicator: u32,
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
    pub const fn offset(&self) -> u64 {
        self.offset
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
    pub const fn is_compressed(&self) -> bool {
        self.len_compressed != self.len_deflated
    }

    pub fn extract<R: Read, W: Write>(&self, reader: &mut R, writer: &mut W) {
        let mut reader = reader.take(self.len_compressed);
        if self.is_compressed() {
            let mut zlib_reader = ZlibDecoder::new(reader);
            io::copy(&mut zlib_reader, writer).expect("failed to copy entry from zlib reader");
        } else {
            io::copy(&mut reader, writer).expect("failed to copy entry from reader");
        }
    }

    /// Writes the contents of `reader` into `writer` to replace the contents of
    /// an entry, performing zlib compression if this entry was originally compressed.
    ///
    /// Returns the number of bytes written to `writer`, functionally equivalent
    /// to `len_compressed`.
    pub fn repack<R: Read, W: Write>(
        &self,
        reader: &mut R,
        writer: &mut W,
        write_padding: bool,
    ) -> u64 {
        let (bytes_written, writer) = if self.is_compressed() {
            let mut zlib_writer = ZlibEncoder::new(writer, Compression::default());
            let bytes_written =
                io::copy(reader, &mut zlib_writer).expect("failed to copy entry from reader");
            let inner_writer = zlib_writer.finish().expect("failed to finish zlib writer");
            (bytes_written, inner_writer)
        } else {
            let bytes_written = io::copy(reader, writer).expect("failed to copy entry from reader");
            (bytes_written, writer)
        };
        
        if write_padding {
            bytes::write_alignment_padding(writer, bytes_written);
        }
        
        bytes_written
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
            cpr_indicator: 0,
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
            cpr_indicator: entry.cpr_indicator,
        }
    }
}
