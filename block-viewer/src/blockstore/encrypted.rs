use anyhow::{Context, Result, bail};
use byte_unit::Byte;
use log::trace;

use super::{
    BlockId, Overhead,
    interface::{BlockStoreReader, LLBlockStore},
};
use cryfs_crypto::symmetric::CipherDef;
use cryfs_utils::data::Data;

// TODO Here and in other files: Add more .context() to errors

const FORMAT_VERSION_HEADER: &[u8; 2] = &1u16.to_ne_bytes();

pub struct EncryptedBlockStore<C: 'static + CipherDef, B> {
    underlying_block_store: B,
    cipher: C,
}

impl<
    // TODO Are all those bounds on C, _B, B still needed ?
    C: CipherDef,
    B,
> EncryptedBlockStore<C, B>
{
    pub fn new(underlying_block_store: B, cipher: C) -> Self {
        Self {
            underlying_block_store,
            cipher: cipher,
        }
    }
}

impl<C: CipherDef, B: BlockStoreReader> BlockStoreReader for EncryptedBlockStore<C, B> {
    fn exists(&self, id: &BlockId) -> Result<bool> {
        self.underlying_block_store.exists(id)
    }

    fn load(&self, id: &BlockId) -> Result<Option<Data>> {
        let loaded = self.underlying_block_store.load(id).context(
            "EncryptedBlockStore failed to load the block from the underlying block store",
        )?;
        match loaded {
            None => Ok(None),
            Some(data) => Ok(Some(self._decrypt(data)?)),
        }
    }

    fn num_blocks(&self) -> Result<u64> {
        self.underlying_block_store.num_blocks()
    }

    fn estimate_num_free_bytes(&self) -> Result<Byte> {
        self.underlying_block_store.estimate_num_free_bytes()
    }

    fn overhead(&self) -> Overhead {
        self.underlying_block_store.overhead()
            + Overhead::new(Byte::from_u64(
                (FORMAT_VERSION_HEADER.len()
                    + C::CIPHERTEXT_OVERHEAD_PREFIX
                    + C::CIPHERTEXT_OVERHEAD_SUFFIX) as u64,
            ))
    }

    fn all_blocks(&self) -> Result<impl Iterator<Item = Result<BlockId>>> {
        self.underlying_block_store.all_blocks()
    }
}

impl<C: CipherDef, B: LLBlockStore> LLBlockStore for EncryptedBlockStore<C, B> {}

impl<C: CipherDef, B> EncryptedBlockStore<C, B> {
    fn _decrypt(&self, ciphertext: Data) -> Result<Data> {
        trace!(
            "== Encrypted data ({}B of overhead) ==",
            FORMAT_VERSION_HEADER.len()
                + C::CIPHERTEXT_OVERHEAD_PREFIX
                + C::CIPHERTEXT_OVERHEAD_SUFFIX
        );
        trace!(
            "Format version header: {} ({}B)",
            1,
            FORMAT_VERSION_HEADER.len()
        );
        trace!("Cipher prefix(Nonce): {}B", C::CIPHERTEXT_OVERHEAD_PREFIX);
        trace!("Cipher suffix(AEAD): {}", C::CIPHERTEXT_OVERHEAD_SUFFIX);
        let ciphertext = _check_and_remove_header(ciphertext)?;
        self.cipher.decrypt(ciphertext)
    }
}

fn _check_and_remove_header(mut data: Data) -> Result<Data> {
    if !data.starts_with(FORMAT_VERSION_HEADER) {
        bail!(
            "Couldn't parse encrypted block. Expected FORMAT_VERSION_HEADER of {:?} but found {:?}",
            FORMAT_VERSION_HEADER,
            &data[..FORMAT_VERSION_HEADER.len()]
        );
    }
    data.shrink_to_subregion(FORMAT_VERSION_HEADER.len()..);
    Ok(data)
}
