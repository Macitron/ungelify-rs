use clap::{Parser, Subcommand};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
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
        #[arg(value_name = "ARCHIVE", help = "Path to the archive")]
        archive_path: PathBuf,
    },
    #[command(
        about = "Extract file(s) from an archive",
        arg_required_else_help = true,
        aliases = ["x", "ex"])]
    Extract {
        #[arg(value_name = "ARCHIVE", help = "Path to the archive")]
        archive_path: PathBuf,
        #[arg(value_name = "ENTRIES", help = "Entry names/globs/IDs to extract")]
        entries: Vec<String>,
        #[arg(short, long, help = "Output directory for extracted files")]
        output_dir: Option<PathBuf>,
    },
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
            entries,
            output_dir,
        } => {
            assert!(archive_path.is_file());
            let parent_dir = archive_path.parent().unwrap();
            let output_dir = output_dir
                .unwrap_or_else(|| parent_dir.join(ungelify::archive_output_dir(&archive_path)));
            fs::create_dir_all(&output_dir).unwrap();

            let mut reader = BufReader::new(File::open(&archive_path).unwrap());
            let mpk = MagesArchive::build(&mut reader);

            if entries.is_empty() {
                mpk.extract(&mut reader, &output_dir);
            } else {
                mpk.extract_entries(&mut reader, &output_dir, &entries);
            }
        }
    }
}
