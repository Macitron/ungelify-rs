#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::path::{Path, PathBuf};

pub mod mpk;

// If the archive path has an extension, use the stem as the output directory.
// Otherwise, use the archive name with a ".d" suffix.
pub fn archive_output_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    let name = path.as_ref().file_name().unwrap();
    let stem = path.as_ref().file_stem().unwrap();

    let mut output_dir = stem.to_owned();
    if stem == name {
        // doesn't have an extension
        output_dir.push(".d");
    }

    PathBuf::from(output_dir)
}
