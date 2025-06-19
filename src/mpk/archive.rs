use crate::mpk::bytes;
use crate::mpk::bytes::{MpkEntryV1, MpkEntryV2, MpkHeader};
use crate::mpk::entry::MagesEntry;
use crate::mpk::iter::Entries;
use bytesize::ByteSize;
use globset::{Glob, GlobSet, GlobSetBuilder};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct MagesArchive {
    entries: IndexMap<u32, MagesEntry>,
    names_to_ids: HashMap<String, u32>,
    is_old_format: bool,
    // Bookkeeping for repacking
    pub(super) ver_major: u16,
    pub(super) ver_minor: u16,
    pub(super) reported_entry_count: u64, // sometimes it lies
}

impl MagesArchive {
    pub const MPK_SIG: &'static [u8] = b"MPK\0";
    const FIRST_HEADER_OFFSET: u64 = 0x40; // first entry header, aka size of the MPK header

    pub fn build<R: Read>(reader: &mut R) -> Self {
        let header: MpkHeader = bytes::read_struct(reader);
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
                let v1_entry: MpkEntryV1 = bytes::read_struct(reader);
                v1_entry.into()
            } else {
                let v2_entry: MpkEntryV2 = bytes::read_struct(reader);
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
            is_old_format,
            ver_major: header.ver_major,
            ver_minor: header.ver_minor,
            reported_entry_count: header.entry_count,
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
            let cpr_suffix = if entry.is_compressed() { "*" } else { "" };
            println!(
                "{:<5} {:<20} {:<12} 0x{:x}",
                entry.id(),
                entry.name(),
                format!("{}{cpr_suffix}", ByteSize::b(entry.len_deflated())),
                entry.offset()
            );
        }
    }

    // Helps with the actual extraction for an entry since the basic functionality is shared
    // between extract() and extract_entries()
    fn do_extraction<R: Read + Seek, P: AsRef<Path>>(
        entry: &MagesEntry,
        reader: &mut R,
        output_dir: P,
    ) {
        reader.seek(SeekFrom::Start(entry.offset())).unwrap();
        let extract_path = output_dir.as_ref().join(entry.name());
        let mut writer = BufWriter::new(File::create(&extract_path).unwrap());
        entry.extract(reader, &mut writer);
    }

    pub fn extract<R: Read + Seek, P: AsRef<Path>>(&self, reader: &mut R, output_dir: P) {
        self.iter()
            .for_each(|entry| Self::do_extraction(entry, reader, &output_dir));
    }

    // build up efficient structures that we can then query when we run through all the entries
    // to decide which ones to extract
    fn build_search_structures(entries_or_ids: &[String]) -> (GlobSet, HashSet<u32>) {
        let mut globset_builder = GlobSetBuilder::new();
        let mut extract_ids = HashSet::new();
        for entry_name in entries_or_ids {
            if let Ok(id) = entry_name.parse::<u32>() {
                extract_ids.insert(id);
            } else {
                globset_builder.add(Glob::new(entry_name).unwrap());
            }
        }

        (
            globset_builder
                .build()
                .expect("error building entry name globset"),
            extract_ids,
        )
    }

    pub fn extract_entries<R: Read + Seek, P: AsRef<Path>>(
        &self,
        reader: &mut R,
        output_dir: P,
        entries_or_ids: &[String],
    ) {
        let (extract_globset, extract_ids) = Self::build_search_structures(entries_or_ids);
        self.iter()
            .filter(|&entry| {
                extract_ids.contains(&entry.id()) || extract_globset.is_match(entry.name())
            })
            .for_each(|entry| Self::do_extraction(entry, reader, &output_dir));
    }

    fn write_archive_header<W: Write>(&self, writer: &mut W) {
        let header: MpkHeader = self.into();
        bytes::write_struct(writer, &header);
    }

    // map of filename => PathBuf so that we can check whether we need to repack an entry
    // with a given filename and then the path to read the contents from
    fn build_repack_map<P: AsRef<Path>>(rpk_paths: &[P]) -> HashMap<String, PathBuf> {
        rpk_paths
            .iter()
            .map(|p| {
                (
                    p.as_ref()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    p.as_ref().to_path_buf(),
                )
            })
            .collect::<HashMap<_, _>>()
    }

    fn repack_from_file<W: Write>(
        rpk_writer: &mut W,
        entry: &MagesEntry,
        new_offset: u64,
        rpk_path: &PathBuf,
    ) -> MagesEntry {
        let rpk_file = File::open(rpk_path).unwrap();
        let src_len = rpk_path.metadata().unwrap().len();
        let mut rpk_reader = BufReader::new(rpk_file);
        let bytes_written = entry.repack(&mut rpk_reader, rpk_writer);

        entry.updated(new_offset, src_len, bytes_written)
    }

    fn copy_original_entry<R: Read + Seek, W: Write>(
        orig_reader: &mut R,
        rpk_writer: &mut W,
        entry: &MagesEntry,
        new_offset: u64,
    ) -> MagesEntry {
        orig_reader.seek(SeekFrom::Start(entry.offset())).unwrap();
        let mut orig_reader = orig_reader.take(entry.len_compressed());
        let bytes_written = io::copy(&mut orig_reader, rpk_writer).unwrap();

        entry.updated(new_offset, entry.len_deflated(), bytes_written)
    }

    fn repack_entry<R: Read + Seek, W: Write + Seek>(
        orig_reader: &mut R,
        rpk_writer: &mut W,
        rpk_paths: &HashMap<String, PathBuf>,
        entry: &MagesEntry,
    ) -> MagesEntry {
        let cur_pos = rpk_writer.stream_position().unwrap();
        bytes::write_alignment_padding(rpk_writer, cur_pos);

        let new_entry_offset = rpk_writer.stream_position().unwrap();

        if let Some(rpk_path) = rpk_paths.get(entry.name()) {
            Self::repack_from_file(rpk_writer, entry, new_entry_offset, rpk_path)
        } else {
            Self::copy_original_entry(orig_reader, rpk_writer, entry, new_entry_offset)
        }
    }

    fn write_entry_headers<W: Write>(
        &self,
        rpk_writer: &mut W,
        rpk_entries: &IndexMap<u32, MagesEntry>,
    ) {
        if self.is_old_format {
            rpk_entries
                .values()
                .for_each(|rpk_entry| bytes::write_struct(rpk_writer, MpkEntryV1::from(rpk_entry)));
        } else {
            rpk_entries
                .values()
                .for_each(|rpk_entry| bytes::write_struct(rpk_writer, MpkEntryV2::from(rpk_entry)));
        }
    }

    #[allow(clippy::return_self_not_must_use)] // I just wanna repack and be done with it
    pub fn repack_entries<R, W, P>(
        &self,
        orig_reader: &mut R,
        rpk_writer: &mut W,
        rpk_paths: &[P],
    ) -> Self
    where
        R: Read + Seek,
        W: Write + Seek,
        P: AsRef<Path>,
    {
        let rpk_paths = Self::build_repack_map(rpk_paths);

        self.write_archive_header(rpk_writer);

        rpk_writer
            .seek(SeekFrom::Start(self.entries[0].offset()))
            .unwrap();
        let rpk_entries = self
            .iter()
            .map(|entry| {
                let new_entry = Self::repack_entry(orig_reader, rpk_writer, &rpk_paths, entry);
                (entry.id(), new_entry)
            })
            .collect::<IndexMap<_, _>>();

        // go back and fill out the headers
        rpk_writer
            .seek(SeekFrom::Start(Self::FIRST_HEADER_OFFSET))
            .unwrap();
        self.write_entry_headers(rpk_writer, &rpk_entries);

        rpk_writer.flush().unwrap();

        Self {
            entries: rpk_entries,
            names_to_ids: self.names_to_ids.clone(),
            is_old_format: self.is_old_format,
            ver_major: self.ver_major,
            ver_minor: self.ver_minor,
            reported_entry_count: self.reported_entry_count,
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
