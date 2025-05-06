use bincode::config::{Configuration as BincodeConfig, Fixint, LittleEndian};
use bincode::Decode;
use std::ffi::CStr;
use std::io::Read;

#[derive(Debug, Decode)]
pub struct MpkHeader {
    pub(super) signature: [u8; 4],
    _ver_minor: u16,
    pub(super) ver_major: u16,
    pub(super) entry_count: u64,
    _padding: [u8; 0x30],
}

#[derive(Debug, Decode)]
pub struct MpkEntryV1 {
    pub(super) id: u32,
    pub(super) offset: u32,
    pub(super) len_compressed: u32,
    pub(super) len_deflated: u32,
    _padding: [u8; 16],
    //   256 bytes per entry header
    // -  32 bytes for other data
    // = 224 bytes max for string
    pub(super) name: [u8; 224],
}

#[derive(Debug, Decode)]
pub struct MpkEntryV2 {
    _cpr_indicator: u32,
    pub(super) id: u32,
    pub(super) offset: u64,
    pub(super) len_compressed: u64,
    pub(super) len_deflated: u64,
    pub(super) name: [u8; 224],
}

type MpkConfig = BincodeConfig<LittleEndian, Fixint>;

const BINCODE_CONFIG: MpkConfig = bincode::config::standard()
    .with_little_endian()
    .with_fixed_int_encoding();

pub fn read_from_file<D: Decode<()>, R: Read>(reader: &mut R) -> D {
    bincode::decode_from_std_read::<D, MpkConfig, R>(reader, BINCODE_CONFIG)
        .expect("failed to decode")
}

pub fn entry_name_from_bytes(name: &[u8]) -> String {
    CStr::from_bytes_until_nul(name)
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}
