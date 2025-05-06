use std::fs::File;
use std::io::BufReader;
use ungelify::mpk::MagesArchive;

fn main() {
    let mut reader = BufReader::new(File::open("resources/chara.mpk").unwrap());
    let mpk = MagesArchive::build(&mut reader);
    mpk.list_entries();
}
