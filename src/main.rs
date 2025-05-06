use crate::cli::Cli;
use clap::Parser;

mod cli;

fn main() {
    let args = Cli::parse();

    cli::run(args);

    // let mut reader = BufReader::new(File::open("resources/chara.mpk").unwrap());
    // let mut mpk = MagesArchive::build(&mut reader);
    // mpk.list_entries();

    // let entry = mpk.entries.values().next().unwrap();
    // reader.seek(SeekFrom::Start(entry.offset)).unwrap();
    // let mut writer = BufWriter::new(File::create(&entry.name).unwrap());
    // entry.extract(&mut reader, &mut writer);
}
