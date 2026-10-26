use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Cli {
    #[command(flatten, next_help_heading = "Config file options")]
    pub config: Option<ConfigPasswordArg>,
    #[command(flatten, next_help_heading = "Key and cipher options")]
    pub key: Option<CipherKeyArg>,
    /// Path to the CryFS block to be inspected.
    #[arg(short, long)]
    pub path: PathBuf,
}

#[derive(Clone, clap::ValueEnum)]
pub enum CipherCli {
    #[clap(name = "xchacha20-poly1305")]
    XChaCha20Poly1305,
    #[clap(name = "aes-256-gcm")]
    Aes256Gcm,
    #[clap(name = "aes-128-gcm")]
    Aes128Gcm,
}

impl std::fmt::Display for CipherCli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CipherCli::XChaCha20Poly1305 => write!(f, "xchacha20-poly1305"),
            CipherCli::Aes256Gcm => write!(f, "aes-256-gcm"),
            CipherCli::Aes128Gcm => write!(f, "aes-128-gcm"),
        }
    }
}

#[derive(clap::Args, Clone)]
#[group(conflicts_with = "CipherKeyArg")]
pub struct ConfigPasswordArg {
    #[arg(long)]
    /// Path to the cryfs.config file to extract the required cipher and key, this option conflicts with '--cipher'
    pub file: PathBuf,
    #[arg(long)]
    pub password: String,
}

#[derive(clap::Args)]
pub struct CipherKeyArg {
    #[arg(long)]
    pub cipher: CipherCli,
    #[arg(long)]
    pub key: String,
}
