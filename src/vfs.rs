use std::cmp::min;
use std::error::Error;
use std::io;
use std::io::{Read, Write};
use std::path::Path;

pub mod mpk;

pub trait Archive: Sized {
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>>;

    fn list_entries(&self);

    fn extract_entry(&self, entry_name_or_id: &str) -> Result<(), Box<dyn Error>>;

    fn extract_all_entries(&self) -> Result<(), Box<dyn Error>>;

    fn replace_entry<P: AsRef<Path>>(self, path: P) -> Result<Self, Box<dyn Error>>;
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
