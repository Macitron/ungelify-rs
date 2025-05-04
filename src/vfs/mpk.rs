use crate::vfs::error::ArchiveError;
use anyhow::{bail, Result as AnyResult};
use bincode::config::{Configuration as BincodeConfig, Fixint, LittleEndian};
use bincode::error::DecodeError;
use bincode::Decode;
use indexmap::IndexMap;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{BufReader, Seek};
use std::path::Path;

// Structs prefixed with "Mages" are logical representations, while those
// prefixed with "Mpk" are the byte representations to be used with bincode and
// converted between each other for v1 and v2.

#[derive(Debug)]
pub struct MagesArchive {
    entries: IndexMap<u32, MagesEntry>,
}

#[derive(Debug)]
struct MagesEntry {
    id: u32,
    name: String,
    offset: u64,
    len_deflated: u64,
    len_compressed: u64,
    is_compressed: bool,
}

#[derive(Debug, Decode)]
struct MpkHeader {
    signature: [u8; 4],
    _ver_minor: u16,
    ver_major: u16,
    entry_count: u64,
    _padding: [u8; 0x30],
}

#[derive(Debug, Decode)]
struct MpkEntryV1 {
    id: u32,
    offset: u32,
    len_compressed: u32,
    len_deflated: u32,
    _padding: [u8; 16],
    //   256 bytes per entry header
    // -  32 bytes for other data
    // = 224 bytes max for string
    name: [u8; 224],
}

#[derive(Debug, Decode)]
struct MpkEntryV2 {
    compression_indicator: u32,
    id: u32,
    offset: u64,
    len_compressed: u64,
    len_deflated: u64,
    name: [u8; 224],
}

type MpkConfig = BincodeConfig<LittleEndian, Fixint>;
const BINCODE_CONFIG: MpkConfig = bincode::config::standard()
    .with_little_endian()
    .with_fixed_int_encoding();

fn read_from_file<D: Decode<()>>(reader: &mut BufReader<File>) -> Result<D, DecodeError> {
    bincode::decode_from_std_read::<D, MpkConfig, BufReader<File>>(reader, BINCODE_CONFIG)
}

impl MagesArchive {
    const MAGIC: &'static [u8] = b"MPK\0";

    pub fn build<P>(path: P) -> AnyResult<Self>
    where
        P: AsRef<Path>,
    {
        let mut reader = BufReader::new(File::open(path)?);

        let header = read_from_file::<MpkHeader>(&mut reader)?;
        println!("header: {header:#?}");
        if header.signature != Self::MAGIC {
            bail!(ArchiveError::BadSignature(
                String::from_utf8_lossy(&header.signature).into()
            ));
        }
        let is_old_format = header.ver_major == 1;

        #[allow(clippy::cast_possible_truncation)]
        let mut entries = IndexMap::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            // let entry_header_offset = Self::GLOBAL_HEADER_LEN + (idx * MagesEntry::HEADER_LEN);
            println!("reader position: {:#x}", reader.stream_position()?);
            let entry: MagesEntry = if is_old_format {
                read_from_file::<MpkEntryV1>(&mut reader)?.try_into()
            } else {
                read_from_file::<MpkEntryV2>(&mut reader)?.try_into()
            }?;

            // there's a known issue where some archives just straight up lie about how many entries
            // they have and at least one entry header is all 0s.
            // example: chara.mpk in C;C LCC has its last entry header as all 0s, overwriting the
            // valid, already-initialized entry that has ID 0 and resulting in a line that looks
            // like:
            //
            // ID    Name                 Size         Offset
            // 0                          0 B          0x0
            //
            // the easiest way to solve this is just to make sure the offset isn't 0, because no
            // entry will ever be at offset 0 in an archive.
            if entry.offset == 0 {
                continue;
            }

            entries.insert(entry.id, entry);
        }

        Ok(Self { entries })
    }
}

fn entry_name_from_bytes(name: &[u8]) -> String {
    CStr::from_bytes_until_nul(name)
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}

// if we converted *from* a V1 entry, we're not gonna have more than 2^32 for those fields
#[allow(clippy::cast_possible_truncation)]
impl From<MagesEntry> for MpkEntryV1 {
    fn from(entry: MagesEntry) -> Self {
        Self {
            id: entry.id,
            offset: entry.offset as u32,
            len_compressed: entry.len_compressed as u32,
            len_deflated: entry.len_deflated as u32,
            _padding: [0; 16],
            name: entry.name.as_bytes().try_into().unwrap(),
        }
    }
}

impl From<MpkEntryV1> for MagesEntry {
    fn from(entry: MpkEntryV1) -> Self {
        Self {
            id: entry.id,
            name: entry_name_from_bytes(&entry.name),
            offset: u64::from(entry.offset),
            len_deflated: u64::from(entry.len_deflated),
            len_compressed: u64::from(entry.len_compressed),
            is_compressed: entry.len_compressed != entry.len_deflated,
        }
    }
}

impl From<MpkEntryV2> for MagesEntry {
    fn from(entry: MpkEntryV2) -> Self {
        Self {
            id: entry.id,
            name: entry_name_from_bytes(&entry.name),
            offset: entry.offset,
            len_deflated: entry.len_deflated,
            len_compressed: entry.len_compressed,
            is_compressed: entry.compression_indicator != 0,
        }
    }
}
