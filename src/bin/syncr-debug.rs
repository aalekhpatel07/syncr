use clap::Parser;
use clap::command;
use syncr::CheckSum;
use syncr::ChecksumConfig;
use syncr::Checksums;
use syncr::multisearch::Matcher;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::vec::Vec;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "syncr")]
#[command(about = "A simple implementation of the rsync rolling checksum algorithm.", long_about = None)]
pub struct Opts {
    #[clap(
        short,
        long,
        help = "The size of each chunk to calculate a running checksum for.",
        default_value_t = 1000
    )]
    block_size: usize,
    #[clap(short, long, help = "The modulus to use for the checksum.", default_value_t = 1 << 16)]
    modulus: u32,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(name = "diff", about = "Compare two files and output the matching byte offsets.", long_about = None)]
    Diff {
        #[arg(value_name = "FILE", required = true, help = "The file to send.")]
        file_to_update: PathBuf,
        #[arg(value_name = "FILE", required = true, help = "The file to receive.")]
        file_to_update_with: PathBuf,
    },
    #[command(name = "checksum", about = "Calculate the checksums for a file.", long_about = None)]
    Checksum {
        #[arg(required = true, help = "The files to calculate checksums for.")]
        files: Vec<PathBuf>,
        #[clap(short, long, help = "The kind of hash to compute (i.e. strong or weak).", default_value_t = false)]
        strong: bool,
    },
}


pub fn main() {
    let args = Opts::parse();
    let mut writer = BufWriter::new(std::io::stdout());

    match args.command {
        Commands::Checksum { files, strong } => {
            for file in files {
                let buffer = std::fs::read(file).unwrap();

                let config = ChecksumConfig {
                    block_size: args.block_size,
                    modulus: args.modulus,
                };

                let checksum = CheckSum::with_config(&config);
                match strong {
                    true => {
                        for checksum in checksum.strong.checksums(&buffer) {
                            write!(writer, "{}", checksum).unwrap();
                        }
                        writer.flush().unwrap();
                    },
                    false => {
                        for checksum in checksum.weak.checksums(&buffer) {
                            write!(writer, "{}", checksum).unwrap();
                        }
                        writer.flush().unwrap();
                    }
                }
            }
        },
        Commands::Diff { file_to_update, file_to_update_with } => {
            let client_buffer = std::fs::read(file_to_update_with.clone()).unwrap();
            let server_buffer = std::fs::read(file_to_update.clone()).unwrap();

            let config = ChecksumConfig {
                block_size: args.block_size,
                modulus: args.modulus,
            };

            let checksum = CheckSum::with_config(&config);
            let mut matcher = Matcher::default();
            matcher.checksum = checksum;

            matcher.compile(&client_buffer);

            let checksum = CheckSum::with_config(&config);
            let server_checksums = checksum.checksums(&server_buffer);
            let found_matches = matcher.find_matches(server_checksums);

            for (start, end) in found_matches {
                writeln!(
                    writer, 
                    "(owned) {:#?} at {} matches (remote) {:#?} at {}", 
                    file_to_update.display(), 
                    start, 
                    file_to_update_with.display(),
                    end
                )
                .unwrap();
            }
        }
    }
}
