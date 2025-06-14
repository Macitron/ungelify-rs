use crate::mpk::{MagesArchive, MagesEntry};
use bincode::config::{Configuration as BincodeConfig, Fixint, LittleEndian};
use bincode::{Decode, Encode};
use std::ffi::CStr;
use std::io::{Read, Write};

#[derive(Debug, Decode, Encode)]
pub(super) struct MpkHeader {
    pub signature: [u8; 4],
    pub ver_minor: u16,
    pub ver_major: u16,
    pub entry_count: u64,
    _padding: [u8; 0x30],
}

#[derive(Debug, Decode, Encode)]
pub(super) struct MpkEntryV1 {
    pub id: u32,
    pub offset: u32,
    pub len_compressed: u32,
    pub len_deflated: u32,
    _padding: [u8; 16],
    //   256 bytes per entry header
    // -  32 bytes for other data
    // = 224 bytes max for string
    pub name: [u8; 224],
}

#[derive(Debug, Decode, Encode)]
pub(super) struct MpkEntryV2 {
    pub cpr_indicator: u32,
    pub id: u32,
    pub offset: u64,
    pub len_compressed: u64,
    pub len_deflated: u64,
    pub name: [u8; 224],
}

type MpkConfig = BincodeConfig<LittleEndian, Fixint>;

const BINCODE_CONFIG: MpkConfig = bincode::config::standard()
    .with_little_endian()
    .with_fixed_int_encoding();

pub fn read_struct<D: Decode<()>, R: Read>(reader: &mut R) -> D {
    bincode::decode_from_std_read::<D, MpkConfig, R>(reader, BINCODE_CONFIG)
        .expect("failed to decode")
}

pub fn write_struct<E: Encode, W: Write>(writer: &mut W, val: E) {
    bincode::encode_into_std_write::<E, MpkConfig, W>(val, writer, BINCODE_CONFIG)
        .expect("failed to encode");
}

pub fn entry_name_from_bytes(name: &[u8]) -> String {
    CStr::from_bytes_until_nul(name)
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}

// MPK aligns the actual start of each entry's data on offsets of 2048
const PADDING_BUF: [u8; 2048] = [0; 2048];
pub fn write_alignment_padding<W: Write>(writer: &mut W, pos: u64) {
    let remainder = pos % 2048;

    if remainder == 0 {
        return;
    }

    let padding_len = 2048 - remainder as usize;
    writer.write_all(&PADDING_BUF[..padding_len]).unwrap();
}

impl From<&MagesArchive> for MpkHeader {
    fn from(archive: &MagesArchive) -> Self {
        Self {
            signature: {
                let mut sig = [0u8; 4];
                sig.copy_from_slice(MagesArchive::MPK_SIG);
                sig
            },
            ver_minor: archive.ver_minor,
            ver_major: archive.ver_major,
            entry_count: archive.reported_entry_count,
            _padding: [0; 0x30],
        }
    }
}

fn copy_name_bytes(entry_name: &str) -> [u8; 224] {
    let mut name_buf = [0u8; 224];
    name_buf[..entry_name.len()].copy_from_slice(entry_name.as_bytes());
    name_buf
}

impl From<&MagesEntry> for MpkEntryV1 {
    fn from(entry: &MagesEntry) -> Self {
        Self {
            id: entry.id(),
            offset: u32::try_from(entry.offset()).expect("V1 entry offset too large for u32"),
            len_compressed: u32::try_from(entry.len_compressed())
                .expect("V1 entry compressed size too large for u32"),
            len_deflated: u32::try_from(entry.len_deflated())
                .expect("V1 entry size too large for u32"),
            _padding: [0; 16],
            name: copy_name_bytes(entry.name()),
        }
    }
}

impl From<&MagesEntry> for MpkEntryV2 {
    fn from(entry: &MagesEntry) -> Self {
        Self {
            cpr_indicator: entry.cpr_indicator,
            id: entry.id(),
            offset: entry.offset(),
            len_compressed: entry.len_compressed(),
            len_deflated: entry.len_deflated(),
            name: copy_name_bytes(entry.name()),
        }
    }
}
