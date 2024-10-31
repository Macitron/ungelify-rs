pub mod mpk;

use std::error::Error;
use std::io;
use std::io::Read;
use std::path::Path;

pub trait Archive: Sized {
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>>;

    fn list_entries(&self);

    fn extract_entry(&self, entry_name_or_id: &str) -> Result<(), Box<dyn Error>>;

    fn extract_all_entries(&self) -> Result<(), Box<dyn Error>>;
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
