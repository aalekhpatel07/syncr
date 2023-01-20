use syncr::weak_checksum::*;
use clap::Parser;
use std::vec::Vec;
use std::path::PathBuf;
use std::io::Write;
use std::io::BufWriter;


#[derive(Parser, Debug)]
#[command(name = "rsync-checksum")]
#[command(about = "A simple implementation of the rsync rolling checksum algorithm.", long_about = None)]
pub struct Args {
    #[clap(short, long, help = "The size of each chunk to calculate a running checksum for.", default_value_t = 1000)]
    block_size: usize,
    #[clap(short, long, help = "The modulus to use for the checksum.", default_value_t = 1 << 16)]
    modulus: u32,
    #[arg(required = true, help = "The files to calculate checksums for.")]
    files: Vec<PathBuf>
}

pub fn main() {
    let args = Args::parse();
    for file in args.files {
        let buffer = std::fs::read(file).unwrap();
        let mut checksum = 
            RollingCheckSumBuilder::new(&buffer)
            .block_size(args.block_size)
            .modulus(args.modulus)
            .build();

        let mut writer = BufWriter::new(std::io::stdout());
        
        for checksum in checksum.rolling_checksums() {
            write!(writer, "{}", checksum).unwrap();
        }
        writer.flush().unwrap();
    }
}