use crate::vfs::error::ArchiveError;
use crate::vfs::mpk::MagesArchive;
use std::cmp::min;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub mod error;
pub mod mpk;

pub trait Archive: Sized {
    fn list_entries(&self);

    fn extract_entries(
        &self,
        entry_names_or_ids: Option<Vec<String>>,
        output_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn Error>>;

    fn replace_entries<P: AsRef<Path>>(self, paths: &[P]) -> Result<Self, Box<dyn Error>>;
}

// gets the name of the archive file without the extension to use as the extraction directory.
// if the file does not have an extension (or, more specifically, the archive's `file_stem()` is
// the same as the archive), then the directory is the archive's name with a `.d` appended to
// it.
// e.g., '../mpk/script.mpk' -> '../mpk/script'
//       './archive_no_ext' -> './archive_no_ext.d'
fn archive_dir_name<P: AsRef<Path>>(archive_path: P) -> Result<PathBuf, ArchiveError> {
    let parent_dir = archive_path
        .as_ref()
        .parent()
        .ok_or("unable to get parent directory of archive")?;
    let archive_stem = archive_path
        .as_ref()
        .file_stem()
        .ok_or("unable to get archive file stem")?;

    let mut archive_dir = parent_dir.join(archive_stem);
    if archive_path.as_ref() == archive_dir {
        let mut archive_d = archive_path
            .as_ref()
            .file_name()
            .ok_or("unable to get archive file name")?
            .to_os_string();
        archive_d.push(".d");
        archive_dir = parent_dir.join(archive_d);
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

/// Reads exactly `n` bytes from `reader` and writes them to `writer`.
///
/// This function is intended to work both with vanilla readers/writers AND with compression
/// read/write encoders/decoders and thus does not keep track of the number of bytes that have been
/// written; that must be done by the caller.
///
/// The only guarantee made is that all `n` bytes will be consumed and that all consumed bytes will
/// be written through whatever transformations the reader/writer pair performs. The `writer` is
/// *not* flushed upon completion.
///
/// # Errors
///
/// If this function encounters any errors while reading or writing to `reader` or `writer`, then
/// the corresponding [`io::Error`] is returned.
fn write_n_from_reader(
    reader: &mut impl Read,
    writer: &mut impl Write,
    n: u64,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut total_read = 0;

    while total_read < n {
        let read_remaining = n - total_read;
        let to_read = min(read_remaining, buffer.len() as u64);

        reader.read_exact(&mut buffer[..usize::try_from(to_read)?])?;
        total_read += to_read;

        writer.write_all(&buffer[..usize::try_from(to_read)?])?;
    }

    Ok(())
}

fn write_all_from_reader(
    reader: &mut impl Read,
    writer: &mut impl Write,
) -> Result<u64, Box<dyn Error>> {
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut bytes_read = usize::MAX;
    let mut total_written = 0;

    while bytes_read > 0 {
        bytes_read = reader.read(&mut buffer)?;
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
        entry_names_or_ids: Option<Vec<String>>,
        output_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Mpk(mpk) => mpk.extract_entries(entry_names_or_ids, output_dir),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_archive_dir_names() {
        let archive_expectations = vec![
            ("script.mpk", "script"),
            ("chara.mpk", "chara"),
            ("bgm.cpk", "bgm"),
            ("no-ext", "no-ext.d"),
            ("two.ext.dots", "two.ext"),
        ];

        for pair in archive_expectations {
            let actual_dir_name = archive_dir_name(pair.0).unwrap();
            assert_eq!(actual_dir_name, PathBuf::from(pair.1));
        }
    }

    #[test]
    fn file_path_names_work() {
        let filename_expectations = vec![
            ("resources/script.mpk", "script.mpk"),
            ("../gamedata/chara.mpk", "chara.mpk"),
            ("/home/stallman/games/cclcc/music/bgm.mpk", "bgm.mpk"),
            ("op18.mp4", "op18.mp4"),
        ];

        for pair in filename_expectations {
            let path = PathBuf::from(pair.0);
            let actual_filename = path_file_name(&path).unwrap();
            assert_eq!(actual_filename, pair.1);
        }
    }
}
