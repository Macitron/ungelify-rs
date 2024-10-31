use byteorder::{ReadBytesExt, LE};
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::{cmp, fs, io};

fn read_signature(reader: &mut impl Read) -> Result<[u8; 4], io::Error> {
    let mut sig_buf = [0u8; 4];
    reader.read_exact(&mut sig_buf)?;
    Ok(sig_buf)
}

fn read_cstring<R: Read + Seek>(reader: &mut R) -> Result<String, Box<dyn Error>> {
    let mut chars = Vec::new();
    let mut c = [0u8; 1];
    loop {
        reader.read_exact(&mut c)?;
        if &c == b"\0" {
            break String::from_utf8(chars).map_err(Into::into);
        }
        chars.extend_from_slice(&c);
    }
}

#[derive(Debug)]
pub struct MpkArchive<R: Read + Seek> {
    reader: R,
    version: MpkVersion,
    entry_count: u64,
    entries: Vec<MpkEntry>,
}

#[derive(Debug)]
struct MpkVersion {
    major: u16,
    minor: u16,
    is_old_format: bool,
}

#[derive(Debug)]
pub struct MpkEntry {
    id: u32,
    offset: u64,
    name: String,
    len: u64,
    len_compressed: u64,
}

impl<R: Read + Seek> MpkArchive<R> {
    pub const SIGNATURE: &'static [u8] = b"MPK\0";
    pub const FILE_HEADER_LENGTH: u64 = 256;

    pub fn from_mpk(mut mpk_reader: R) -> Result<Self, Box<dyn Error>> {
        let signature = read_signature(&mut mpk_reader)?;
        if signature != Self::SIGNATURE {
            return Err(format!("invalid file signature '{signature:?}' for MPK archive").into());
        }

        let ver_minor = mpk_reader.read_u16::<LE>()?;
        let ver_major = mpk_reader.read_u16::<LE>()?;
        let mpk_version = MpkVersion::build(ver_major, ver_minor)?;

        let entry_count = if mpk_version.is_old_format {
            u64::from(mpk_reader.read_u32::<LE>()?)
        } else {
            mpk_reader.read_u64::<LE>()?
        };

        let first_entry_offset = if mpk_version.is_old_format {
            0x40
        } else {
            0x44
        };

        let mut entries = Vec::with_capacity(usize::try_from(entry_count)?);
        for idx in 0..entry_count {
            let header_entry_offset = first_entry_offset + (idx * Self::FILE_HEADER_LENGTH);
            entries.push(MpkEntry::read_at_offset(
                header_entry_offset,
                &mut mpk_reader,
                mpk_version.is_old_format,
            )?);
        }

        Ok(Self {
            reader: mpk_reader,
            version: mpk_version,
            entry_count,
            entries,
        })
    }

    pub fn list_entries(&self) {
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

    // extracts entry specified by `entry_name_or_id` to a file. `path` is the path to the actual
    // .mpk file, a folder with the name of the MPK minus the extension will be created
    pub fn extract_entry(
        &mut self,
        path: &Path,
        entry_name_or_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mpk_dir = path.parent().unwrap().join(path.file_stem().unwrap());
        fs::create_dir_all(&mpk_dir)?;

        let entry = self.get_entry(entry_name_or_id).unwrap();
        let entry_name = entry.name.clone();
        let entry_path = mpk_dir.join(&entry.name);

        let entry_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&entry_path)?;
        let mut entry_writer = BufWriter::new(entry_file);

        let mut buffer = vec![0u8; 1024 * 16]; // good enough. might profile later
        let mut total_written = 0;
        let entry_len = usize::try_from(entry.len)?;

        self.reader.seek(SeekFrom::Start(entry.offset))?;
        while total_written < entry_len {
            let bytes_remaining = entry_len - total_written;
            let to_read = cmp::min(bytes_remaining, buffer.len());
            let bytes_read = self.reader.read(&mut buffer[..to_read])?;

            let bytes_written = entry_writer.write(&buffer[..bytes_read])?;
            total_written += bytes_written;
        }
        entry_writer.flush()?;

        if total_written == entry_len {
            Ok(())
        } else {
            Err(format!("failed to extract entry file '{entry_name}'").into())
        }
    }

    pub fn get_entry(&self, entry_name_or_id: &str) -> Option<&MpkEntry> {
        entry_name_or_id.parse::<u32>().map_or_else(
            |_| self.get_entry_by_name(entry_name_or_id),
            |id| self.get_entry_by_id(id),
        )
    }

    fn get_entry_by_id(&self, id: u32) -> Option<&MpkEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn get_entry_by_name(&self, name: &str) -> Option<&MpkEntry> {
        self.entries
            .iter()
            .find(|e| e.name.to_lowercase() == name.to_lowercase())
    }
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
}

impl MpkEntry {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

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

        let name = read_cstring(mpk_reader)?;

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
}
