use clap::{Parser, Subcommand};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use ungelify::mpk::MagesArchive;

#[derive(Debug, Parser)]
#[command(
    name = "ungelify",
    version,
    about = "A CLI tool for extracting and repacking MAGES archives."
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
        #[arg(value_name = "ARCHIVE", help = "The path to the archive.")]
        archive_path: PathBuf,
    },
    #[command(
        about = "Extract file(s) from an archive",
        arg_required_else_help = true,
        aliases = ["x", "ex"])]
    Extract {
        #[arg(value_name = "ARCHIVE", help = "The path to the archive.")]
        archive_path: PathBuf,
        #[arg(
            value_name = "ENTRIES",
            help = "Choose specific entry names/globs/IDs to extract."
        )]
        entries: Vec<String>,
        #[arg(
            short,
            long,
            help = "The output directory for extracted files.\nWill be created if it does not exist."
        )]
        output_dir: Option<PathBuf>,
    },
    #[command(
        about = "Repack files to a new archive",
        arg_required_else_help = true,
        aliases = ["replace", "r", "re"])]
    Repack {
        #[arg(value_name = "ARCHIVE", help = "The path to the archive.")]
        archive_path: PathBuf,
        #[arg(
            value_name = "REPACK_FILES",
            help = "A list of file paths to repack the new archive with."
        )]
        rpk_files: Vec<PathBuf>,
        #[arg(
            short,
            long,
            help = "Do not save a backup copy of the original archive."
        )]
        no_save: bool,
    },
}

fn append_to_path(p: impl Into<OsString>, s: impl AsRef<OsStr>) -> PathBuf {
    let mut p = p.into();
    p.push(s);
    p.into()
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
        Cmd::Repack {
            archive_path,
            rpk_files,
            no_save,
        } => {
            assert!(archive_path.is_file());
            let orig_path = append_to_path(&archive_path, ".orig");
            fs::rename(&archive_path, &orig_path).unwrap();

            let mut orig_reader = BufReader::new(File::open(&orig_path).unwrap());
            let mpk = MagesArchive::build(&mut orig_reader);
            let mut rpk_writer = BufWriter::new(File::create(&archive_path).unwrap());

            mpk.repack_entries(&mut orig_reader, &mut rpk_writer, &rpk_files);

            if no_save {
                fs::remove_file(&orig_path).unwrap();
            }
        }
    }
}
