use anyhow::Result;
use clap::Parser;
use cryfs_config_viewer::{ConfigLoader, LogMode};
use std::{fs::File, io::BufReader, path::PathBuf};

#[derive(Parser)]
struct Args {
    /// Password to decrypt the cryfs.config with
    #[arg(short, long)]
    password: String,
    /// Path to the cryfs.config file
    #[arg(short, long, value_name = "CONFIG")]
    file: PathBuf,
    /// Will prettify the json output if present (doesn't display the original formatting)
    #[arg(long)]
    prettify: bool,
    /// Will only display the inner config JSON
    #[arg(short, long)]
    inner: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let log_mode = if args.inner {
        LogMode::InnerConfig
    } else {
        LogMode::FullConfig
    };
    let loader = ConfigLoader::with_log_output(log_mode, args.prettify);

    let password = &args.password;
    let mut reader: std::io::BufReader<File> = BufReader::new(File::open(args.file)?);
    loader.print_config(&mut reader, password)
}
