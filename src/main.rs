use ungelify::vfs::mpk::MpkArchive;
use ungelify::vfs::Archive;

fn main() {
    let filename: String = "resources/script.mpk".to_string();
    let mpk: MpkArchive = Archive::from_file(filename).unwrap();

    mpk.list_entries();

    mpk.extract_entry("sg03_03.scx").unwrap();
    mpk.extract_all_entries().unwrap();
}
