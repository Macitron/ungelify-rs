use byteorder::{ReadBytesExt, LE};
use std::error::Error;
use std::io;
use std::io::{Read, Seek, SeekFrom};

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
            break String::from_utf8(chars).map_err(|e| e.into());
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
struct MpkEntry {
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
            return Err(format!("invalid file signature '{:?}' for MPK archive", signature).into());
        }

        let ver_minor = mpk_reader.read_u16::<LE>()?;
        let ver_major = mpk_reader.read_u16::<LE>()?;
        let mpk_version = MpkVersion::build(ver_major, ver_minor)?;

        let entry_count = if mpk_version.is_old_format {
            mpk_reader.read_u32::<LE>()? as u64
        } else {
            mpk_reader.read_u64::<LE>()?
        };

        let first_entry_offset = if mpk_version.is_old_format {
            0x40
        } else {
            0x44
        };

        let mut entries = Vec::with_capacity(entry_count as usize);
        for idx in 0..entry_count {
            let header_entry_offset = first_entry_offset + (idx * Self::FILE_HEADER_LENGTH);
            entries.push(MpkEntry::read_at_offset(
                header_entry_offset,
                &mut mpk_reader,
                mpk_version.is_old_format,
            )?);
        }

        Ok(MpkArchive {
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
}

impl MpkVersion {
    fn build(major: u16, minor: u16) -> Result<Self, String> {
        if major != 1 && major != 2 {
            Err(format!("unsupported MPK archive version {major}"))
        } else {
            Ok(MpkVersion {
                major,
                minor,
                is_old_format: major == 1,
            })
        }
    }
}

impl MpkEntry {
    fn read_at_offset<R: Read + Seek>(
        offset: u64,
        mpk_reader: &mut R,
        is_old_format: bool,
    ) -> Result<MpkEntry, Box<dyn Error>> {
        mpk_reader.seek(SeekFrom::Start(offset))?;

        let id = mpk_reader.read_u32::<LE>()?;
        let offset: u64;
        let len_compressed: u64;
        let len_uncompressed: u64;

        if is_old_format {
            offset = mpk_reader.read_u32::<LE>()? as u64;
            len_compressed = mpk_reader.read_u32::<LE>()? as u64;
            len_uncompressed = mpk_reader.read_u32::<LE>()? as u64;
            mpk_reader.seek(SeekFrom::Current(16))?;
        } else {
            offset = mpk_reader.read_u64::<LE>()?;
            len_compressed = mpk_reader.read_u64::<LE>()?;
            len_uncompressed = mpk_reader.read_u64::<LE>()?;
        }

        let name = read_cstring(mpk_reader)?;

        Ok(MpkEntry {
            id,
            offset,
            name,
            len: len_uncompressed,
            len_compressed,
        })
    }

    fn is_compressed(&self) -> bool {
        self.len == self.len_compressed
    }
}
