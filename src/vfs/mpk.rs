use crate::vfs;
use crate::vfs::Archive;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::cell::RefCell;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug)]
pub struct MpkArchive {
    reader: RefCell<BufReader<File>>,
    file_path: PathBuf,
    version: MpkVersion,
    entry_count: u64,
    entries: Vec<MpkEntry>,
}

impl MpkArchive {
    pub const SIGNATURE: &'static [u8] = b"MPK\0";

    fn get_entry<'a>(entries: &'a [MpkEntry], entry_name_or_id: &str) -> Option<&'a MpkEntry> {
        entry_name_or_id.parse::<u32>().map_or_else(
            |_| Self::get_entry_by_name(entries, entry_name_or_id),
            |id| Self::get_entry_by_id(entries, id),
        )
    }

    fn get_entry_by_id(entries: &[MpkEntry], entry_id: u32) -> Option<&MpkEntry> {
        entries.iter().find(|e| e.id == entry_id)
    }

    fn get_entry_by_name<'a>(entries: &'a [MpkEntry], entry_name: &str) -> Option<&'a MpkEntry> {
        entries
            .iter()
            .find(|e| e.name.to_lowercase() == entry_name.to_lowercase())
    }
}

impl Archive for MpkArchive {
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let file = OpenOptions::new().read(true).open(&path)?;
        let mut reader = BufReader::new(file);

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

        let mut entries = Vec::with_capacity(usize::try_from(entry_count)?);
        for idx in 0..entry_count {
            let entry_header_offset =
                version.first_entry_header_offset() + (idx * MpkEntry::HEADER_LENGTH);
            let entry =
                MpkEntry::read_at_offset(entry_header_offset, &mut reader, version.is_old_format)?;
            entries.push(entry);
        }

        Ok(Self {
            reader: RefCell::new(reader),
            file_path: path.as_ref().to_path_buf(),
            version,
            entry_count,
            entries,
        })
    }

    fn list_entries(&self) {
        // maybe want to calculate the actual longest ID length, longest filename length rather than
        // using magic constants
        println!("\n{:<5} {:<20} {}", "ID", "Name", "Size");

        for entry in &self.entries {
            println!(
                "{:<5} {:<20} {}",
                entry.id,
                entry.name,
                bytesize::to_string(entry.len, true)
            );
        }
    }

    fn extract_entry(&self, entry_name_or_id: &str) -> Result<(), Box<dyn Error>> {
        // create directory with the name of the file minus the extension
        // e.g. 'script.mpk' -> 'script/'
        let mpk_dir = self
            .file_path
            .parent()
            .unwrap()
            .join(self.file_path.file_stem().unwrap());
        fs::create_dir_all(&mpk_dir)?;

        let entry = Self::get_entry(&self.entries, entry_name_or_id).unwrap();
        entry.extract(&mut *self.reader.borrow_mut(), &mpk_dir)
    }

    fn extract_all_entries(&self) -> Result<(), Box<(dyn Error)>> {
        let mpk_dir = self
            .file_path
            .parent()
            .unwrap()
            .join(self.file_path.file_stem().unwrap());
        fs::create_dir_all(&mpk_dir)?;

        for entry in &self.entries {
            entry.extract(&mut *self.reader.borrow_mut(), &mpk_dir)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
struct MpkVersion {
    major: u16,
    minor: u16,
    is_old_format: bool,
}

impl MpkVersion {
    fn build(major: u16, minor: u16) -> Result<Self, String> {
        if major != 1 && major != 2 {
            Err(format!("unsupported MPK archive version {major}"))
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
pub struct MpkEntry {
    id: u32,
    offset: u64,
    name: String,
    len: u64,
    len_compressed: u64,
}

impl MpkEntry {
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

    const fn is_compressed(&self) -> bool {
        self.len == self.len_compressed
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

        let padding_len = Self::HEADER_LENGTH - writer.stream_position()?;
        vfs::write_padding(writer, usize::try_from(padding_len)?)?;

        Ok(())
    }

    // consumes this entry and writes a new one at the offset that `writer` is currently at upon
    // invocation.
    // if `is_replacing` is true, then all the contents of `reader` will be written to `writer` and
    // treated as the new contents of the entry; otherwise, `reader` is treated as the existing MPK
    // archive and the existing contents of the entry will simply be copied over.
    fn write_new<R: Read + Seek, W: Write + Seek>(
        self,
        reader: &mut R,
        writer: &mut W,
        is_replacing: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let new_offset = writer.stream_position()?;

        let len_written = if is_replacing {
            reader.seek(SeekFrom::Start(self.offset))?;
            vfs::copy_n(reader, writer, usize::try_from(self.len_compressed)?)?
        } else {
            io::copy(reader, writer)?
        };

        // pad to align on 2048-byte blocks
        if len_written % 2048 != 0 || len_written == 0 {
            // number of blocks it would take to fit `len_written`
            // (round up to nearest multiple of 2048)
            let num_blocks = len_written / 2048 + 1;
            let len_with_padding = num_blocks * 2048;
            vfs::write_padding(writer, usize::try_from(len_with_padding - len_written)?)?;
        }

        Ok(Self {
            offset: new_offset,
            len: len_written, // eventually rework for compressed entries
            len_compressed: len_written,
            ..self
        })
    }
}
