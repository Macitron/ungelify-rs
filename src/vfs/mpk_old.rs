use crate::vfs;
use crate::vfs::error::ArchiveError;
use crate::vfs::Archive;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use flate2::bufread::ZlibEncoder as ZlibEncodeReader;
use flate2::write::ZlibDecoder as ZlibDecodeWriter;
use flate2::Compression;
use globset::GlobSetBuilder;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug)]
pub struct MagesArchive {
    reader: RefCell<BufReader<File>>,
    file_path: PathBuf,
    version: MpkVersion,
    entry_count: u64,
    entries: IndexMap<u32, MagesEntry>,
    entry_name_map: HashMap<String, u32>,
}

impl MagesArchive {
    pub const SIGNATURE: &'static [u8] = b"MPK\0";

    const FIRST_ENTRY_HEADER_OFFSET: u64 = 0x40;

    pub fn build<P: AsRef<Path>>(
        mut reader: BufReader<File>,
        path: P,
    ) -> Result<Self, Box<dyn Error>> {
        let signature = vfs::read_signature(&mut reader)?;
        if signature != Self::SIGNATURE {
            return Err(format!("invalid file signature '{signature:?}' for MPK archive").into());
        }

        let ver_minor = reader.read_u16::<LE>()?;
        let ver_major = reader.read_u16::<LE>()?;
        let version = MpkVersion::build(ver_major, ver_minor)?;

        let entry_count = if version.is_old_format {
            u64::from(reader.read_u32::<LE>()?)
        } else {
            reader.read_u64::<LE>()?
        };

        let mut entries = IndexMap::with_capacity(usize::try_from(entry_count)?);
        let mut entry_name_map = HashMap::with_capacity(entries.capacity());
        for idx in 0..entry_count {
            let entry_header_offset =
                Self::FIRST_ENTRY_HEADER_OFFSET + (idx * MagesEntry::HEADER_LENGTH);
            reader.seek(SeekFrom::Start(entry_header_offset))?;
            let entry = MagesEntry::read(&mut reader, &version)?;

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

            let (entry_id, entry_name) = (entry.id, entry.name.clone());
            entries.insert(entry_id, entry);
            entry_name_map.insert(entry_name, entry_id);
        }

        Ok(Self {
            reader: RefCell::new(reader),
            file_path: path.as_ref().to_path_buf(),
            version,
            entry_count,
            entries,
            entry_name_map,
        })
    }

    // get all entries that match the globs given
    // - build up a glob set from the cli args
    // - then iterate over all entries, only add those which match the globset ezpz
    // TODO error or log when one or more globs don't match any entries
    fn get_entries(
        &self,
        entry_globs_or_ids: Vec<String>,
    ) -> Result<Vec<&MagesEntry>, Box<dyn Error>> {
        let mut results = vec![];
        let mut globset = GlobSetBuilder::new();

        for glob in entry_globs_or_ids {
            if let Ok(id) = glob.parse::<u32>() {
                results.push(
                    self.get_entry_by_id(id)
                        .ok_or_else(|| format!("entry with ID {id} not found in archive"))?,
                );
            } else {
                globset.add(glob.parse()?);
            }
        }

        let glob_set = globset.build()?;
        if !glob_set.is_empty() {
            for entry_name in self.entry_name_map.keys() {
                if glob_set.is_match(entry_name) {
                    results.push(self.get_entry_by_name(entry_name).ok_or_else(|| {
                        format!("no entry with name {entry_name} found in archive")
                    })?);
                }
            }
        }

        Ok(results)
    }

    fn get_entry_by_id(&self, entry_id: u32) -> Option<&MagesEntry> {
        self.entries.get(&entry_id)
    }

    fn get_entry_by_name(&self, entry_name: &str) -> Option<&MagesEntry> {
        let entry_id = self.entry_name_map.get(entry_name)?;
        self.entries.get(entry_id)
    }

    // finds the path in `paths` that has the name `entry_name`, if it exists
    fn find_entry_path_match<'a>(
        entry_name: &str,
        paths: &'a [impl AsRef<Path>],
    ) -> Option<&'a Path> {
        for path in paths {
            let filename = vfs::path_file_name(path.as_ref()).ok()?;
            if entry_name == filename {
                return Some(path.as_ref());
            }
        }

        None
    }

    // write the archive header (signature, version, entry count) as well as the padding needed to
    // get up to the first entry's data
    fn write_archive_preamble<W: Write + Seek>(
        &self,
        writer: &mut W,
    ) -> Result<(), Box<dyn Error>> {
        // look into using bincode or bytemuck crates for directly reading/writing structs
        writer.write_all(Self::SIGNATURE)?;
        writer.write_u16::<LE>(self.version.minor)?;
        writer.write_u16::<LE>(self.version.major)?;

        if self.version.is_old_format {
            writer.write_u32::<LE>(u32::try_from(self.entry_count)?)?;
        } else {
            writer.write_u64::<LE>(self.entry_count)?;
        }

        Ok(())
    }

    // if `entry.len_compressed`, i.e. the number of bytes written to the archive, is not aligned on
    // a block of 2048, write padding until it is
    fn write_entry_alignment_padding<W: Write>(
        writer: &mut W,
        len_written: u64,
    ) -> Result<(), Box<dyn Error>> {
        if len_written % 2048 == 0 && len_written != 0 {
            // already aligned, nothing to do
            return Ok(());
        }

        // number of blocks it would take to fit `len_written`
        // (round up to nearest multiple of 2048)
        let num_blocks = len_written / 2048 + 1;
        let len_with_padding = num_blocks * 2048;
        vfs::write_padding(writer, usize::try_from(len_with_padding - len_written)?)?;

        Ok(())
    }
}

impl Archive for MagesArchive {
    #[allow(clippy::print_literal)] // readability >>>
    fn list_entries(&self) {
        // maybe want to calculate the actual longest ID length, longest filename length rather than
        // using magic constants
        println!("{:<5} {:<20} {:<12} {}", "ID", "Name", "Size", "Offset");

        for entry in self.entries.values() {
            println!(
                "{:<5} {:<20} {:<12} {:#x}",
                entry.id,
                entry.name,
                bytesize::to_string(entry.len, true) + if entry.is_compressed() { "*" } else { "" },
                entry.offset
            );
        }
    }

    fn extract_entries(
        &self,
        entry_names_or_ids: Option<Vec<String>>,
        output_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn Error>> {
        let entries_to_extract = match entry_names_or_ids {
            Some(entry_globs) => self.get_entries(entry_globs)?,
            None => self.entries.values().collect(),
        };

        if entries_to_extract.is_empty() {
            return Err(ArchiveError::from("provided patterns did not match any entries").into());
        }

        let extract_dir = match output_dir {
            Some(path) => path,
            None => vfs::archive_dir_name(&self.file_path)?,
        };
        fs::create_dir_all(&extract_dir)?;

        for entry in entries_to_extract {
            entry.extract(&mut *self.reader.borrow_mut(), &extract_dir)?;
        }

        Ok(())
    }

    // look into `glob` crate for replacing multiple files

    // this one could probably use some work. right now it's O(N^2) for N entries in the archive,
    // since for each entry it searches the whole array of paths for which one to replace, and I
    // can't come up with a better algo rn. maybe a HashSet would help
    fn replace_entries<P: AsRef<Path>>(self, paths: &[P]) -> Result<Self, Box<dyn Error>> {
        if paths.is_empty() {
            return Err("no replacement files were specified".into());
        }

        for path in paths {
            let filename = vfs::path_file_name(path.as_ref())?;
            if !self.entry_name_map.contains_key(filename) {
                return Err(format!("entry '{filename}' does not exist in archive").into());
            }
        }

        let new_file = tempfile::tempfile()?;
        let mut temp_writer = BufWriter::new(new_file);

        self.write_archive_preamble(&mut temp_writer)?;

        // write out all the entries in the data portion of the archive, building up the new Vec of
        // entries as we go, then afterwards come back and write the entry headers
        let first_entry_offset = self.entries[0].offset;
        let padding_length = first_entry_offset - temp_writer.stream_position()?;
        vfs::write_padding(&mut temp_writer, usize::try_from(padding_length)?)?;

        // we don't need a new entry-name map because this is only a repacking application.
        // `entry.write_new()` doesn't change the id OR the name, just the offset and length of data
        let mut new_entries = IndexMap::with_capacity(usize::try_from(self.entry_count)?);

        let mut entry_iter = self.entries.into_iter().peekable();
        while let Some((_, entry)) = entry_iter.next() {
            let opt_new_path = Self::find_entry_path_match(&entry.name, paths);
            let new_entry = if let Some(path) = opt_new_path {
                // replacing entry with contents of a source file
                let file = File::open(path)?;
                let file_len = file.metadata()?.len();
                let mut reader = BufReader::new(file);

                if entry.is_compressed() {
                    let mut reader = ZlibEncodeReader::new(reader, Compression::default());
                    entry.write_new(&mut reader, &mut temp_writer, Some(file_len))?
                } else {
                    entry.write_new(&mut reader, &mut temp_writer, Some(file_len))?
                }
            } else {
                // no replacement, write entry contents from existing archive to new one
                let mut reader = &mut *self.reader.borrow_mut();
                reader.seek(SeekFrom::Start(entry.offset))?;
                entry.write_new(&mut reader, &mut temp_writer, None)?
            };

            // last file in the archive does not have to be aligned on a 2048-byte block
            // ... for some reason
            if entry_iter.peek().is_some() {
                Self::write_entry_alignment_padding(&mut temp_writer, new_entry.len_compressed)?;
            }

            new_entries.insert(new_entry.id, new_entry);
        }

        // maybe refactor so we create the entries vector first with all the metadata and then write
        // the entry data after?
        // leads to kind of a chicken-or-egg problem where the offsets are ill-defined til they're
        // actually written. maybe we can get clever with it and calculate the length + number of
        // blocks ahead of time, but that might not play well with compression idk
        temp_writer.seek(SeekFrom::Start(Self::FIRST_ENTRY_HEADER_OFFSET))?;
        for entry in new_entries.values() {
            entry.write_header(&mut temp_writer, &self.version)?;
        }

        // overwrite contents of the MPK with the temp file
        temp_writer.flush()?;
        let mut temp_reader = BufReader::new(temp_writer.into_inner()?);
        temp_reader.seek(SeekFrom::Start(0))?;

        let mpk_writer = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.file_path)?;
        let mut mpk_writer = BufWriter::new(mpk_writer);

        // should we just move the temp file to the MPK? but that might have complications with
        // file permissions
        io::copy(&mut temp_reader, &mut mpk_writer)?;

        Ok(Self {
            reader: RefCell::new(BufReader::new(File::open(&self.file_path)?)),
            entries: new_entries,
            ..self
        })
    }
}

#[derive(Debug)]
struct MpkVersion {
    major: u16,
    minor: u16,
    is_old_format: bool,
}

impl MpkVersion {
    fn build(major: u16, minor: u16) -> Result<Self, ArchiveError> {
        if major != 1 && major != 2 {
            Err(format!("unsupported MPK archive version {major}").into())
        } else {
            Ok(Self {
                major,
                minor,
                is_old_format: major == 1,
            })
        }
    }
}

#[derive(Debug)]
pub struct MagesEntry {
    // not entirely clear how these first 4 bytes of the entry header is used, but it seems to be
    // 1u32 if the entry is compressed and 0 otherwise. alignment shenanigans?
    // for v1 archives this doesn't exist, the header starts right at the entry id
    compression_indicator: u32,
    id: u32,
    offset: u64,
    // might want to revisit eventually, reading/writing raw bytes as a UTF-8 string seems hairy
    name: String,
    len: u64,
    len_compressed: u64,
}

impl MagesEntry {
    const HEADER_LENGTH: u64 = 256;

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    fn read<R: Read + Seek>(
        mpk_reader: &mut R,
        version: &MpkVersion,
    ) -> Result<Self, Box<dyn Error>> {
        let compression_indicator = if version.is_old_format {
            0
        } else {
            mpk_reader.read_u32::<LE>()?
        };

        let id = mpk_reader.read_u32::<LE>()?;
        let offset: u64;
        let len_compressed: u64;
        let len_uncompressed: u64;

        if version.is_old_format {
            offset = u64::from(mpk_reader.read_u32::<LE>()?);
            len_compressed = u64::from(mpk_reader.read_u32::<LE>()?);
            len_uncompressed = u64::from(mpk_reader.read_u32::<LE>()?);
            mpk_reader.seek_relative(16)?;
        } else {
            offset = mpk_reader.read_u64::<LE>()?;
            len_compressed = mpk_reader.read_u64::<LE>()?;
            len_uncompressed = mpk_reader.read_u64::<LE>()?;
        }

        let name = vfs::read_cstring(mpk_reader)?;

        Ok(Self {
            compression_indicator,
            id,
            offset,
            name,
            len: len_uncompressed,
            len_compressed,
        })
    }

    const fn is_compressed(&self) -> bool {
        self.compression_indicator != 0
    }

    fn extract<R: Read + Seek, P: AsRef<Path>>(
        &self,
        reader: &mut R,
        target_dir: P,
    ) -> Result<(), Box<dyn Error>> {
        let entry_path = target_dir.as_ref().join(self.name());
        let entry_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&entry_path)?;
        println!(
            "ungelify: extracting{} file '{}'",
            if self.is_compressed() {
                " compressed"
            } else {
                ""
            },
            self.name()
        );

        let mut writer = BufWriter::new(entry_file);

        reader.seek(SeekFrom::Start(self.offset))?;
        if self.is_compressed() {
            // println!(
            //     "{} is compressed: len={}, len_compressed={}",
            //     self.name(),
            //     self.len,
            //     self.len_compressed
            // );
            let mut zlib_writer = ZlibDecodeWriter::new(writer);
            vfs::write_n_from_reader(reader, &mut zlib_writer, self.len_compressed)?;
            // eprintln!("wrote {} bytes to file", zlib_writer.total_out());
            zlib_writer.finish()?;
        } else {
            vfs::write_n_from_reader(reader, &mut writer, self.len)?;
            writer.flush()?;
        }

        Ok(())
    }

    fn write_header<W: Write + Seek>(
        &self,
        writer: &mut W,
        version: &MpkVersion,
    ) -> Result<(), Box<dyn Error>> {
        let header_offset = writer.stream_position()?;

        if !version.is_old_format {
            writer.write_u32::<LE>(self.compression_indicator)?;
        }

        writer.write_u32::<LE>(self.id)?;

        if version.is_old_format {
            writer.write_u32::<LE>(u32::try_from(self.offset)?)?;
            writer.write_u32::<LE>(u32::try_from(self.len_compressed)?)?;
            writer.write_u32::<LE>(u32::try_from(self.len)?)?;
            writer.seek_relative(16)?;
        } else {
            writer.write_u64::<LE>(self.offset)?;
            writer.write_u64::<LE>(self.len_compressed)?;
            writer.write_u64::<LE>(self.len)?;
        }

        vfs::write_cstring(writer, self.name())?;

        let bytes_written = writer.stream_position()? - header_offset;
        let padding_len = Self::HEADER_LENGTH - bytes_written;
        vfs::write_padding(writer, usize::try_from(padding_len)?)?;

        Ok(())
    }

    // consumes this entry and writes a new one at the offset that `writer` is currently at upon
    // invocation.
    //
    // block alignment is NOT performed by this function and must be ensured by the caller.
    fn write_new<R: Read, W: Write + Seek>(
        self,
        reader: &mut R,
        writer: &mut W,
        source_len: Option<u64>, // None if not replacing with contents of a file
    ) -> Result<Self, Box<dyn Error>> {
        println!(
            "ungelify: writing{} file '{}'",
            if self.is_compressed() {
                " compressed"
            } else {
                ""
            },
            self.name()
        );

        let new_offset = writer.stream_position()?;

        let len_uncompressed;
        if let Some(len) = source_len {
            vfs::write_all_from_reader(reader, writer)?;
            len_uncompressed = len;
        } else {
            vfs::write_n_from_reader(reader, writer, self.len_compressed)?;
            len_uncompressed = self.len;
        };

        let bytes_written = writer.stream_position()? - new_offset;

        Ok(Self {
            offset: new_offset,
            len: len_uncompressed,
            len_compressed: bytes_written,
            ..self
        })
    }
}
