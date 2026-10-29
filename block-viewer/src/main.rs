use std::{fs::File, io::BufReader, path::PathBuf};

use anyhow::Result;
use blockstore::{
    EncryptedBlockStore, IntegrityBlockStore, OnDiskBlockStore, low_level::BlockStoreReader,
};
use clap::Parser;
use cryfs_config_viewer::ConfigLoader;
use cryfs_crypto::symmetric::{Aes128Gcm, Aes256Gcm, CipherDef, EncryptionKey, XChaCha20Poly1305};

use crate::{
    blockstore::BlockId,
    cli::{CipherKeyArg, ConfigPasswordArg},
};
mod blobstore;
mod blockstore;
mod cli;

fn load_block<C: CipherDef + 'static>(path: PathBuf, key: EncryptionKey) -> Result<()> {
    let root = path.parent().unwrap().parent().unwrap().to_path_buf();

    let on_disk = OnDiskBlockStore::new(root);
    let cipher = C::new(key)?;
    let encrypted = EncryptedBlockStore::new(on_disk, cipher);
    let integrity = IntegrityBlockStore::new(encrypted);

    let components: Vec<_> = path.components().collect();
    let id_components = components.windows(2).last().unwrap();
    let id = id_components.iter().fold(String::new(), |mut acc, comp| {
        acc.push_str(&comp.as_os_str().to_string_lossy());
        acc
    });
    let id = BlockId::from_hex(&id)?;

    integrity.load(&id)?;

    Ok(())
}

fn main() -> Result<()> {
    let _logger = colog::basic_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();
    let args = cli::Cli::parse();
    let (cipher_name, hex_key) = match (args.config, args.key) {
        (Some(ConfigPasswordArg { file, password }), None) => {
            let mut reader = BufReader::new(File::open(file)?);
            let loader = ConfigLoader::new();
            let config = loader.load_config(&mut reader, &password)?;
            (config.cipher, config.enc_key)
        }
        (None, Some(CipherKeyArg { cipher, key })) => (cipher.to_string(), key),
        _ => unreachable!("Both arguments are required by clap and they conflict with each other."),
    };
    let enc_key = EncryptionKey::from_hex(&hex_key)?;
    let path = args.path;
    match cipher_name.as_str() {
        "xchacha20-poly1305" => load_block::<XChaCha20Poly1305>(path, enc_key),
        "aes-256-gcm" => load_block::<Aes256Gcm>(path, enc_key),
        "aes-128-gcm" => load_block::<Aes128Gcm>(path, enc_key),
        _ => panic!("Shouldn't be any other cipher options"),
    };

    Ok(())
}
