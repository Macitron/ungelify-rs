use clap::{Parser, Subcommand};
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use ungelify::mpk::MagesArchive;

#[derive(Debug, Parser)]
#[command(
    name = "ungelify",
    version,
    about = "A CLI tool for extracting and repacking Mages/Criware archives"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(
        about = "List out contents of an archive",
        arg_required_else_help = true,
        aliases = ["l", "ls", "list-contents"])]
    List {
        #[arg(value_name = "ARCHIVE", help = "The archive to list")]
        archive_path: PathBuf,
    },
    #[command(
        about = "Extract file(s) from an archive",
        arg_required_else_help = true,
        aliases = ["x", "ex"])]
    Extract {
        #[arg(value_name = "ARCHIVE", help = "The archive to extract")]
        archive_path: PathBuf,
        #[arg(short, long, help = "Output directory for extracted files")]
        output_dir: Option<PathBuf>,
    },
}

// If the archive path has an extension, use the stem as the output directory.
// Otherwise, use the archive name with a ".d" suffix.
fn archive_output_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    let name = path.as_ref().file_name().unwrap();
    let stem = path.as_ref().file_stem().unwrap();

    let mut output_dir = stem.to_owned();
    if stem == name {
        // doesn't have an extension
        output_dir.push(".d");
    }

    PathBuf::from(output_dir)
}

pub fn run(cli: Cli) {
    match cli.command {
        Cmd::List { archive_path } => {
            assert!(archive_path.is_file());
            let mut reader = BufReader::new(File::open(&archive_path).unwrap());
            let mpk = MagesArchive::build(&mut reader);
            mpk.list_entries();
        }
        Cmd::Extract {
            archive_path,
            output_dir,
        } => {
            assert!(archive_path.is_file());
            let parent_dir = archive_path.parent().unwrap();
            let output_dir =
                output_dir.unwrap_or_else(|| parent_dir.join(archive_output_dir(&archive_path)));
            fs::create_dir_all(&output_dir).unwrap();

            let mut reader = BufReader::new(File::open(&archive_path).unwrap());
            let mpk = MagesArchive::build(&mut reader);
            for entry in &mpk {
                reader.seek(SeekFrom::Start(entry.offset())).unwrap();
                let extract_path = output_dir.join(entry.name());
                let mut writer = BufWriter::new(File::create(&extract_path).unwrap());
                entry.extract(&mut reader, &mut writer);
            }
        }
    }
}
