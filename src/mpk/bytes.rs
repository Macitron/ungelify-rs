use crate::mpk::{MagesArchive, MagesEntry};
use bincode::config::{Configuration as BincodeConfig, Fixint, LittleEndian};
use bincode::{Decode, Encode};
use std::ffi::CStr;
use std::io::{Read, Write};

#[derive(Debug, Decode, Encode)]
pub(super) struct MpkHeader {
    pub signature: [u8; 4],
    _ver_minor: u16,
    pub ver_major: u16,
    pub entry_count: u64,
    _padding: [u8; 0x30],
}

#[derive(Debug, Decode)]
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

#[derive(Debug, Decode)]
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

pub fn read_from_file<D: Decode<()>, R: Read>(reader: &mut R) -> D {
    bincode::decode_from_std_read::<D, MpkConfig, R>(reader, BINCODE_CONFIG)
        .expect("failed to decode")
}

pub fn write_struct<E: Encode, W: Write>(writer: &mut W, value: E) {
    bincode::encode_into_std_write::<E, MpkConfig, W>(value, writer, BINCODE_CONFIG)
        .expect("failed to encode");
}

const PADDING_BUF: [u8; 2048] = [0; 2048];
pub fn write_alignment_padding<W: Write>(writer: &mut W, bytes_written: u64) {
    if bytes_written % 2048 == 0 && bytes_written != 0 {
        return;
    }
    
    let padding_len = 2048 - (bytes_written % 2048) as usize;
    writer.write_all(&PADDING_BUF[..padding_len]).unwrap();
}

pub fn entry_name_from_bytes(name: &[u8]) -> String {
    CStr::from_bytes_until_nul(name)
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}

impl From<&MagesArchive> for MpkHeader {
    fn from(ar: &MagesArchive) -> Self {
        Self {
            signature: MagesArchive::MPK_SIG.try_into().unwrap(),
            _ver_minor: ar.ver_minor,
            ver_major: ar.ver_major,
            entry_count: ar.og_entry_count,
            _padding: [0; 0x30],
        }
    }
}

impl From<MagesEntry> for MpkEntryV1 {
    fn from(entry: MagesEntry) -> Self {
        let mut ret = Self {
            id: entry.id(),
            offset: u32::try_from(entry.offset()).expect("V1 entry offset too large for u32"),
            len_compressed: u32::try_from(entry.len_compressed())
                .expect("V1 entry compressed size too large for u32"),
            len_deflated: u32::try_from(entry.len_deflated())
                .expect("V1 entry size too large for u32"),
            _padding: [0; 16],
            name: [0; 224],
        };
        ret.name[..entry.name().len()].copy_from_slice(entry.name().as_bytes());

        ret
    }
}

impl From<MagesEntry> for MpkEntryV2 {
    fn from(entry: MagesEntry) -> Self {
        let mut ret = Self {
            cpr_indicator: entry.cpr_indicator,
            id: entry.id(),
            offset: entry.offset(),
            len_compressed: entry.len_compressed(),
            len_deflated: entry.len_deflated(),
            name: [0; 224],
        };
        ret.name[..entry.name().len()].copy_from_slice(entry.name().as_bytes());

        ret
    }
}
