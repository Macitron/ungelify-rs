use crate::mpk::bytes;
use crate::mpk::bytes::{MpkEntryV1, MpkEntryV2, MpkHeader};
use crate::mpk::entry::MagesEntry;
use bytesize::ByteSize;
use indexmap::IndexMap;
use std::io::Read;

#[derive(Debug)]
pub struct MagesArchive {
    pub(crate) entries: IndexMap<u32, MagesEntry>,
}

impl MagesArchive {
    const MPK_SIG: &'static [u8] = b"MPK\0";

    pub fn build<R: Read>(reader: &mut R) -> Self {
        let header: MpkHeader = bytes::read_from_file(reader);
        assert_eq!(header.signature, Self::MPK_SIG, "invalid MPK signature");
        let is_old_format = header.ver_major == 1;

        // if usize is 32 and there's (somehow) more than 2^32 entries, we at
        // least want to give it the most capacity possible
        #[allow(clippy::cast_possible_truncation)]
        let mut entries = IndexMap::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            let entry: MagesEntry = if is_old_format {
                let v1_entry: MpkEntryV1 = bytes::read_from_file(reader);
                v1_entry.into()
            } else {
                let v2_entry: MpkEntryV2 = bytes::read_from_file(reader);
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
            if entry.offset() == 0 {
                continue;
            }

            entries.insert(entry.id(), entry);
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
                entry.id(),
                entry.name(),
                ByteSize::b(entry.len_deflated()),
                entry.offset()
            );
        }
    }
}
