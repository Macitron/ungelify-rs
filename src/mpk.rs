use bincode::config::{Configuration as BincodeConfig, Fixint, LittleEndian};
use bincode::Decode;
use bytesize::ByteSize;
use indexmap::IndexMap;
use std::ffi::CStr;
use std::io::Read;

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

// Structs prefixed with Mpk are byte representations of data as they appear in .mpk files, to be
// used and deserialized with bincode.

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
    _cpr_indicator: u32,
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

fn read_from_file<D: Decode<()>, R: Read>(reader: &mut R) -> D {
    bincode::decode_from_std_read::<D, MpkConfig, R>(reader, BINCODE_CONFIG)
        .expect("failed to decode")
}

impl MagesArchive {
    const MPK_SIG: &'static [u8] = b"MPK\0";

    pub fn build<R: Read>(reader: &mut R) -> Self {
        let header: MpkHeader = read_from_file(reader);
        assert_eq!(header.signature, Self::MPK_SIG, "invalid MPK signature");
        let is_old_format = header.ver_major == 1;

        // if usize is 32 and there's (somehow) more than 2^32 entries, we at
        // least want to give it the most capacity possible
        #[allow(clippy::cast_possible_truncation)]
        let mut entries = IndexMap::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            let entry: MagesEntry = if is_old_format {
                let v1_entry: MpkEntryV1 = read_from_file(reader);
                v1_entry.into()
            } else {
                let v2_entry: MpkEntryV2 = read_from_file(reader);
                v2_entry.into()
            };

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

        Self { entries }
    }

    #[allow(clippy::print_literal)] // readability >>>
    pub fn list_entries(&self) {
        println!("{:<5} {:<20} {:<12} {}", "ID", "Name", "Size", "Offset");
        println!("================================================");

        // TODO make an iterator for entries
        for entry in self.entries.values() {
            println!(
                "{:<5} {:<20} {:<12} 0x{:x}",
                entry.id,
                entry.name,
                ByteSize::b(entry.len_deflated),
                entry.offset
            );
        }
    }
}

fn entry_name_from_bytes(name: &[u8]) -> String {
    CStr::from_bytes_until_nul(name)
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}

impl From<MpkEntryV1> for MagesEntry {
    fn from(entry: MpkEntryV1) -> Self {
        Self {
            id: entry.id,
            name: entry_name_from_bytes(&entry.name),
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
            name: entry_name_from_bytes(&entry.name),
            offset: entry.offset,
            len_deflated: entry.len_deflated,
            len_compressed: entry.len_compressed,
            is_compressed: entry.len_compressed != entry.len_deflated,
        }
    }
}
