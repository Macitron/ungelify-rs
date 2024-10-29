use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use ungelify::vfs::MpkArchive;

fn main() {
    let filename: String = "resources/script.mpk".to_string();
    // let filename: String = "resources/bad-ver.mpk".to_string();

    let filepath = Path::new(&filename);

    let mpk_file =
        File::open(&filepath).expect(format!("could not open file '{}'", filename).as_str());
    // eprintln!("opened file '{filename}': {mpk_file:?}");

    let mut reader = BufReader::new(mpk_file);

    let mpk = MpkArchive::from_mpk(&mut reader).unwrap();

    eprintln!("loaded MPK archive: {mpk:#?}");

    mpk.list_entries();
}
