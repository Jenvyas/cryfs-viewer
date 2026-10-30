use std::{fs::File, io::BufReader, path::PathBuf};

use anyhow::Result;
use blobstore::BlobStoreOnBlocks;
use blockstore::{
    AbstractBlockStore, BlockId, EncryptedBlockStore, IntegrityBlockStore, OnDiskBlockStore,
};
use byte_unit::Byte;
use clap::Parser;
use cryfs_config_viewer::ConfigLoader;
use cryfs_crypto::symmetric::{Aes128Gcm, Aes256Gcm, CipherDef, EncryptionKey, XChaCha20Poly1305};

use cli::{CipherKeyArg, ConfigPasswordArg};
mod blobstore;
use blobstore::BlobStore;
use log::trace;

use crate::blobstore::{BlobId, FsBlob, FsBlobStore};
mod blockstore;
mod cli;

fn load_block<C: CipherDef + 'static>(
    path: PathBuf,
    key: EncryptionKey,
    block_size: Byte,
    root_blob_id: BlobId,
) -> Result<()> {
    let root = path.parent().unwrap().parent().unwrap().to_path_buf();

    let on_disk = OnDiskBlockStore::new(root);
    let cipher = C::new(key)?;
    let encrypted = EncryptedBlockStore::new(on_disk, cipher);
    let integrity = IntegrityBlockStore::new(encrypted);
    let abstract_store = AbstractBlockStore::new(integrity);
    let blobstore = BlobStoreOnBlocks::new(abstract_store, block_size)?;
    let fs_store = FsBlobStore::new(blobstore);

    let components: Vec<_> = path.components().collect();
    let id_components = components.windows(2).last().unwrap();
    let id = id_components.iter().fold(String::new(), |mut acc, comp| {
        acc.push_str(&comp.as_os_str().to_string_lossy());
        acc
    });
    let id = BlockId::from_hex(&id)?;
    if id == root_blob_id.root {
        Err(anyhow::Error::msg(
            "Block to inspect should be different from the root block.",
        ))?
    }

    let root_blob = fs_store.load(&root_blob_id)?.unwrap();
    if let FsBlob::Directory(root_dir) = root_blob {
        'outer: for entry in root_dir.entries() {
            let loaded_entry = fs_store.load(entry.blob_id())?.unwrap();
            for loaded_id in loaded_entry.all_blocks()?.flatten() {
                if loaded_id == id {
                    trace!("Block {} is part of blob {}.", id, entry.blob_id());
                    trace!("Blob type: {:?}", entry.entry_type());
                    trace!("Blob directory entry name: {}", entry.name());
                    break 'outer;
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let _logger = colog::basic_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();
    let args = cli::Cli::parse();
    let (cipher_name, hex_key, block_size, root_id) = match (args.config, args.key) {
        (Some(ConfigPasswordArg { file, password }), None) => {
            let mut reader = BufReader::new(File::open(file)?);
            let loader = ConfigLoader::new();
            let config = loader.load_config(&mut reader, &password)?;
            (
                config.cipher,
                config.enc_key,
                config.blocksize,
                BlobId::from_hex(&config.root_blob)?,
            )
        }
        (
            None,
            Some(CipherKeyArg {
                cipher,
                key,
                block_size,
                root_blob_id,
            }),
        ) => (
            cipher.to_string(),
            key,
            Byte::from_u64(block_size),
            BlobId::from_hex(&root_blob_id)?,
        ),
        _ => unreachable!("Both arguments are required by clap and they conflict with each other."),
    };
    let enc_key = EncryptionKey::from_hex(&hex_key)?;
    let path = args.path;
    match cipher_name.as_str() {
        "xchacha20-poly1305" => load_block::<XChaCha20Poly1305>(path, enc_key, block_size, root_id),
        "aes-256-gcm" => load_block::<Aes256Gcm>(path, enc_key, block_size, root_id),
        "aes-128-gcm" => load_block::<Aes128Gcm>(path, enc_key, block_size, root_id),
        _ => panic!("Shouldn't be any other cipher options"),
    }?;

    Ok(())
}
