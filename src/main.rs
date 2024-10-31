use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use ungelify::vfs::mpk::MpkArchive;

fn main() {
    let filename: String = "resources/script.mpk".to_string();
    // let filename: String = "resources/bad-ver.mpk".to_string();

    let filepath = Path::new(&filename);

    let mpk_file =
        File::open(&filepath).expect(format!("could not open file '{}'", filename).as_str());
    // eprintln!("opened file '{filename}': {mpk_file:?}");

    let mut reader = BufReader::new(mpk_file);

    let mut mpk = MpkArchive::from_mpk(&mut reader).unwrap();

    eprintln!("loaded MPK archive: {mpk:#?}");

    mpk.list_entries();

    eprintln!("filepath parent is {:?}", filepath.parent().unwrap());
    eprintln!("filepath stem is {:?}", filepath.file_stem().unwrap());

    // SG03_03.SCX
    mpk.extract_entry(filepath, "sg03_03.scx").unwrap();
}
