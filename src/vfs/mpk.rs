use crate::vfs;
use crate::vfs::error::ArchiveError;
use crate::vfs::Archive;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
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

    // the amount of padding needed between the end of the header info (signature, version, header
    // count) and the start of the entry headers. the entry header section always starts at either
    // 0x40 or 0x44 (for v1 or v2, respectively), so we can calculate how much padding is needed to
    // get there:
    //
    // old                          new
    //   4 bytes signature            4 bytes signature
    // + 2 bytes minor ver          + 2 bytes minor ver
    // + 2 bytes major ver          + 2 bytes major ver
    // + 4 bytes entry count        + 8 bytes entry count
    // = 12 bytes                   = 16 bytes
    //
    // first entry offset = 0x40 = 64 if old
    //                    = 0x44 = 68 if new
    //
    // padding = 64 - 12 = 52 if old
    //         = 68 - 16 = 52 if new
    //
    // so we always need 52 bytes of 0 padding.
    const HEADER_PADDING: [u8; 52] = [0u8; 52];

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
                version.first_entry_header_offset() + (idx * MagesEntry::HEADER_LENGTH);
            let entry = MagesEntry::read_at_offset(
                entry_header_offset,
                &mut reader,
                version.is_old_format,
            )?;

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

    fn get_entry(&self, entry_name_or_id: &str) -> Result<&MagesEntry, ArchiveError> {
        entry_name_or_id.parse::<u32>().map_or_else(
            |_| {
                Self::get_entry_by_name(&self.entries, &self.entry_name_map, entry_name_or_id)
                    .ok_or_else(|| {
                        format!("entry '{entry_name_or_id}' not found in archive").into()
                    })
            },
            |id| {
                Self::get_entry_by_id(&self.entries, id)
                    .ok_or_else(|| format!("entry with ID {id} not found in archive").into())
            },
        )
    }

    fn get_entry_by_id(entries: &IndexMap<u32, MagesEntry>, entry_id: u32) -> Option<&MagesEntry> {
        entries.get(&entry_id)
    }

    fn get_entry_by_name<'a>(
        entries: &'a IndexMap<u32, MagesEntry>,
        entry_name_map: &HashMap<String, u32>,
        entry_name: &str,
    ) -> Option<&'a MagesEntry> {
        let entry_id = entry_name_map.get(entry_name)?;
        entries.get(entry_id)
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

        writer.write_all(&Self::HEADER_PADDING)?;

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
                bytesize::to_string(entry.len, true),
                entry.offset
            );
        }
    }

    fn extract_entries(
        &self,
        entry_names_or_ids: &[String],
        output_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn Error>> {
        let mut entries_to_extract = vec![];
        for identifier in entry_names_or_ids {
            let found_entry = self.get_entry(identifier)?;
            entries_to_extract.push(found_entry);
        }

        let extract_dir = match output_dir {
            Some(path) => {
                fs::create_dir_all(&path)?;
                path
            }
            None => vfs::create_archive_dir(&self.file_path)?,
        };

        for entry in entries_to_extract {
            entry.extract(&mut *self.reader.borrow_mut(), &extract_dir)?;
        }

        Ok(())
    }

    fn extract_all_entries(&self, output_dir: Option<PathBuf>) -> Result<(), Box<(dyn Error)>> {
        let extract_dir = match output_dir {
            Some(path) => {
                fs::create_dir_all(&path)?;
                path
            }
            None => vfs::create_archive_dir(&self.file_path)?,
        };
        for entry in self.entries.values() {
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
            let reader = match opt_new_path {
                Some(path) => &mut BufReader::new(File::open(path)?),
                None => &mut *self.reader.borrow_mut(),
            };

            let is_replacing = opt_new_path.is_some();
            let new_entry = entry.write_new(reader, &mut temp_writer, is_replacing)?;
            let len_written = new_entry.len_compressed;
            new_entries.insert(new_entry.id, new_entry);

            // last file in the archive does not have to be aligned on a 2048-byte block
            // ... for some reason
            if entry_iter.peek().is_some() {
                Self::write_entry_alignment_padding(&mut temp_writer, len_written)?;
            }
        }

        // maybe refactor so we create the entries vector first with all the metadata and then write
        // the entry data after?
        // leads to kind of a chicken-or-egg problem where the offsets are ill-defined til they're
        // actually written. maybe we can get clever with it and calculate the length + number of
        // blocks ahead of time, but that might not play well with compression idk
        temp_writer.seek(SeekFrom::Start(self.version.first_entry_header_offset()))?;
        for entry in new_entries.values() {
            entry.write_header(&mut temp_writer, self.version.is_old_format)?;
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

    const fn first_entry_header_offset(&self) -> u64 {
        if self.is_old_format {
            0x40
        } else {
            0x44
        }
    }
}

#[derive(Debug)]
pub struct MagesEntry {
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

    // TODO add that off-by-one check that dex mentioned (check if offset is garbo)
    fn read_at_offset<R: Read + Seek>(
        offset: u64,
        mpk_reader: &mut R,
        is_old_format: bool,
    ) -> Result<Self, Box<dyn Error>> {
        mpk_reader.seek(SeekFrom::Start(offset))?;

        let id = mpk_reader.read_u32::<LE>()?;
        let offset: u64;
        let len_compressed: u64;
        let len_uncompressed: u64;

        if is_old_format {
            offset = u64::from(mpk_reader.read_u32::<LE>()?);
            len_compressed = u64::from(mpk_reader.read_u32::<LE>()?);
            len_uncompressed = u64::from(mpk_reader.read_u32::<LE>()?);
            mpk_reader.seek(SeekFrom::Current(16))?;
        } else {
            offset = mpk_reader.read_u64::<LE>()?;
            len_compressed = mpk_reader.read_u64::<LE>()?;
            len_uncompressed = mpk_reader.read_u64::<LE>()?;
        }

        let name = vfs::read_cstring(mpk_reader)?;

        Ok(Self {
            id,
            offset,
            name,
            len: len_uncompressed,
            len_compressed,
        })
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
        let mut writer = BufWriter::new(entry_file);

        reader.seek(SeekFrom::Start(self.offset))?;
        vfs::copy_n(reader, &mut writer, usize::try_from(self.len)?)?;
        writer.flush()?;

        Ok(())
    }

    fn write_header<W: Write + Seek>(
        &self,
        writer: &mut W,
        is_old_format: bool,
    ) -> Result<(), Box<dyn Error>> {
        let header_offset = writer.stream_position()?;

        writer.write_u32::<LE>(self.id)?;

        if is_old_format {
            writer.write_u32::<LE>(u32::try_from(self.offset)?)?;
            writer.write_u32::<LE>(u32::try_from(self.len_compressed)?)?;
            writer.write_u32::<LE>(u32::try_from(self.len)?)?;
            writer.seek(SeekFrom::Current(16))?;
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
    // if `is_replacing` is true, then all the contents of `reader` will be written to `writer` and
    // treated as the new contents of the entry; otherwise, `reader` is treated as the existing MPK
    // archive and the existing contents of the entry will simply be copied over.
    //
    // block alignment is NOT performed by this function and must be ensured by the caller.
    fn write_new<R: Read + Seek, W: Write + Seek>(
        self,
        reader: &mut R,
        writer: &mut W,
        is_replacing: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let new_offset = writer.stream_position()?;

        let len_written = if is_replacing {
            io::copy(reader, writer)?
        } else {
            reader.seek(SeekFrom::Start(self.offset))?;
            vfs::copy_n(reader, writer, usize::try_from(self.len_compressed)?)?
        };

        Ok(Self {
            offset: new_offset,
            len: len_written, // eventually rework for compressed entries
            len_compressed: len_written,
            ..self
        })
    }
}
