use crate::vfs::error::ArchiveError;
use crate::vfs::mpk::MagesArchive;
use std::cmp::min;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

pub mod error;
pub mod mpk;

pub trait Archive: Sized {
    fn list_entries(&self);

    fn extract_entries(
        &self,
        entry_names_or_ids: &[String],
        output_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn Error>>;

    fn extract_all_entries(&self, output_dir: Option<PathBuf>) -> Result<(), Box<dyn Error>>;

    fn replace_entries<P: AsRef<Path>>(self, paths: &[P]) -> Result<Self, Box<dyn Error>>;
}

// gets the name of the archive file without the extension to use as the extraction directory.
// if the file does not have an extension (or, more specifically, the archive's `file_stem()` is
// the same as the archive), then the directory is the archive's name with a `.d` appended to
// it.
// e.g., '../mpk/script.mpk' -> '../mpk/script'
//       './archive_no_ext' -> './archive_no_ext.d'
fn create_archive_dir(archive_path: &Path) -> Result<PathBuf, ArchiveError> {
    let parent_dir = archive_path
        .parent()
        .ok_or("unable to get parent directory of archive")?;
    let archive_stem = archive_path
        .file_stem()
        .ok_or("unable to get archive file stem")?;

    let mut archive_dir = parent_dir.join(archive_stem);
    if archive_path == archive_dir {
        let mut archive_d = archive_path
            .file_name()
            .ok_or("unable to get archive file name")?
            .to_os_string();
        archive_d.push(".d");
        archive_dir = parent_dir.join(archive_d);
    }

    if let Err(e) = fs::create_dir_all(&archive_dir) {
        return Err(format!("error creating directory {archive_dir:?} for archive: {e}",).into());
    }

    Ok(archive_dir)
}

fn path_file_name(path: &Path) -> Result<&str, ArchiveError> {
    let filename_str = path
        .file_name()
        .ok_or_else(|| format!("unable to get OsStr filename of {path:?}"))?
        .to_str()
        .ok_or_else(|| format!("unable to get unicode filename of {path:?}"))?;

    Ok(filename_str)
}

fn read_signature(reader: &mut impl Read) -> Result<[u8; 4], io::Error> {
    let mut sig_buf = [0u8; 4];
    reader.read_exact(&mut sig_buf)?;
    Ok(sig_buf)
}

fn read_cstring(reader: &mut impl Read) -> Result<String, Box<dyn Error>> {
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

fn write_cstring(writer: &mut impl Write, string: &str) -> Result<(), Box<dyn Error>> {
    writer.write_all(string.as_bytes())?;
    writer.write_all(b"\0")?;

    Ok(())
}

const BUFFER_SIZE: usize = 1024 * 8; // can fine-tune later

// write `padding_length` zero bytes to the writer
fn write_padding<W: Write>(writer: &mut W, padding_length: usize) -> Result<(), io::Error> {
    let zero_buffer = [0u8; BUFFER_SIZE];
    let mut total_written = 0usize;

    while total_written < padding_length {
        let left_to_write = padding_length - total_written;
        let current_len = min(left_to_write, zero_buffer.len());

        let bytes_written = writer.write(&zero_buffer[..current_len])?;
        total_written += bytes_written;
    }

    Ok(())
}

// copy `n` bytes from `reader` to `writer`
// does not flush! (bad roommate)
fn copy_n(reader: &mut impl Read, writer: &mut impl Write, n: usize) -> Result<u64, io::Error> {
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut total_written = 0;

    while total_written < n {
        let bytes_remaining = n - total_written;
        let to_read = min(bytes_remaining, buffer.len());

        let bytes_read = reader.read(&mut buffer[..to_read])?;
        writer.write_all(&buffer[..bytes_read])?;

        total_written += bytes_read;
    }

    Ok(total_written as u64)
}

pub enum ArchiveImpl {
    Mpk(MagesArchive),
}

impl ArchiveImpl {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let mut reader = BufReader::new(File::open(&path)?);
        let sig = read_signature(&mut reader)?;

        reader.seek(SeekFrom::Start(0))?;
        match &sig[..] {
            MagesArchive::SIGNATURE => Ok(MagesArchive::build(reader, path)?.into()),
            _ => Err(format!(
                "unrecognized archive signature '{}'",
                String::from_utf8_lossy(&sig)
            )
            .into()),
        }
    }
}

impl Archive for ArchiveImpl {
    fn list_entries(&self) {
        match self {
            Self::Mpk(mpk) => mpk.list_entries(),
        }
    }

    fn extract_entries(
        &self,
        entry_names_or_ids: &[String],
        output_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Mpk(mpk) => mpk.extract_entries(entry_names_or_ids, output_dir),
        }
    }

    fn extract_all_entries(&self, output_dir: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Mpk(mpk) => mpk.extract_all_entries(output_dir),
        }
    }

    fn replace_entries<P: AsRef<Path>>(self, paths: &[P]) -> Result<Self, Box<dyn Error>> {
        match self {
            Self::Mpk(mpk) => Ok(mpk.replace_entries(paths)?.into()),
        }
    }
}

impl From<MagesArchive> for ArchiveImpl {
    fn from(value: MagesArchive) -> Self {
        Self::Mpk(value)
    }
}
