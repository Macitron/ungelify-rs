use crate::mpk::bytes;
use crate::mpk::bytes::{MpkEntryV1, MpkEntryV2, MpkHeader};
use crate::mpk::entry::MagesEntry;
use crate::mpk::iter::Entries;
use bytesize::ByteSize;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug)]
pub struct MagesArchive {
    entries: IndexMap<u32, MagesEntry>,
    names_to_ids: HashMap<String, u32>,
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
        #[allow(clippy::cast_possible_truncation)]
        let mut names_to_ids = HashMap::with_capacity(header.entry_count as usize);

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

            names_to_ids.insert(entry.name().to_string(), entry.id());
            entries.insert(entry.id(), entry);
        }

        Self {
            entries,
            names_to_ids,
        }
    }

    #[must_use]
    pub fn iter(&self) -> Entries {
        Entries::new(&self.entries)
    }

    #[must_use]
    pub fn get_entry_by_id(&self, id: u32) -> Option<&MagesEntry> {
        self.entries.get(&id)
    }

    #[must_use]
    pub fn get_entry_by_name(&self, name: &str) -> Option<&MagesEntry> {
        self.names_to_ids
            .get(name)
            .and_then(|id| self.get_entry_by_id(*id))
    }

    #[allow(clippy::print_literal)] // readability >>>
    pub fn list_entries(&self) {
        println!("{:<5} {:<20} {:<12} {}", "ID", "Name", "Size", "Offset");
        println!("================================================");

        for entry in self {
            // TODO implement Display for entries
            println!(
                "{:<5} {:<20} {:<12} 0x{:x}",
                entry.id(),
                entry.name(),
                ByteSize::b(entry.len_deflated()),
                entry.offset()
            );
        }
    }

    pub fn extract<R: Read + Seek, P: AsRef<Path>>(&self, reader: &mut R, output_dir: P) {
        for entry in self {
            reader.seek(SeekFrom::Start(entry.offset())).unwrap();
            let extract_path = output_dir.as_ref().join(entry.name());
            let mut writer = BufWriter::new(File::create(&extract_path).unwrap());
            entry.extract(reader, &mut writer);
        }
    }
}

impl<'a> IntoIterator for &'a MagesArchive {
    type Item = &'a MagesEntry;
    type IntoIter = Entries<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
