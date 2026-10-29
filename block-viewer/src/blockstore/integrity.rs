use anyhow::{Context, Result, bail, ensure};
use binary_layout::prelude::*;
use binrw::{BinRead, BinWrite};
use byte_unit::Byte;
use log::trace;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, fmt::Debug, num::NonZeroU32};
use thiserror::Error;

use super::{
    BLOCKID_LEN, BlockId, Overhead,
    low_level::{BlockStoreReader, LLBlockStore},
};

use cryfs_utils::{binary::read_nonzerou32, data::Data};

#[derive(Error, Debug, PartialEq, Clone)]
pub enum IntegrityViolationError {
    #[error(
        "Integrity Violation: {id_from_header:?} is stored as {id_from_filename:?}. Did an attacker try to rename some blocks?"
    )]
    WrongBlockId {
        id_from_filename: BlockId,
        id_from_header: BlockId,
    },
}

const FORMAT_VERSION_HEADER: u16 = 1;

#[derive(PartialEq, Eq, Hash, BinRead, Clone, Copy, Serialize, Deserialize)]
#[brw(little)]
pub struct ClientId {
    // TODO Tuple struct would be better but https://github.com/jam1garner/binwrite/issues/3
    #[br(parse_with = read_nonzerou32)]
    pub id: NonZeroU32, // NonZeroU32 so we can efficiently store MaybeClientId
}

impl binary_layout::LayoutAs<NonZeroU32> for ClientId {
    type ReadError = Infallible;
    type WriteError = Infallible;

    fn try_read(id: NonZeroU32) -> Result<ClientId, Infallible> {
        Ok(ClientId { id })
    }

    fn try_write(id: ClientId) -> Result<NonZeroU32, Infallible> {
        Ok(id.id)
    }
}

impl std::fmt::Debug for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClientId({})", self.id)
    }
}

#[derive(PartialEq, Eq, Hash, PartialOrd, BinRead, BinWrite, Clone, Copy)]
pub struct BlockVersion {
    // TODO Tuple struct would be better but https://github.com/jam1garner/binwrite/issues/3
    pub version: u64,
}

impl std::fmt::Display for BlockVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl binary_layout::LayoutAs<u64> for BlockVersion {
    type ReadError = Infallible;
    type WriteError = Infallible;

    fn try_read(version: u64) -> Result<BlockVersion, Infallible> {
        Ok(BlockVersion { version })
    }

    fn try_write(version: BlockVersion) -> Result<u64, Infallible> {
        Ok(version.version)
    }
}

binary_layout::binary_layout!(block_layout, LittleEndian, {
    // TODO Use types BlockId, FormatVersionHeader as types instead of slices
    format_version_header: u16,
    block_id: [u8; BLOCKID_LEN],
    last_update_client_id: ClientId as NonZeroU32,
    block_version: BlockVersion as u64,
    data: [u8],
});

const HEADER_SIZE_BYTES: usize = block_layout::data::OFFSET;

pub struct IntegrityBlockStore<B> {
    underlying_block_store: B,
}

impl<B> IntegrityBlockStore<B> {
    pub fn new(underlying_block_store: B) -> Self {
        Self {
            underlying_block_store,
        }
    }
}

impl<B: BlockStoreReader> BlockStoreReader for IntegrityBlockStore<B> {
    fn exists(&self, block_id: &BlockId) -> Result<bool> {
        self.underlying_block_store.exists(block_id)
    }

    fn load(&self, block_id: &BlockId) -> Result<Option<Data>> {
        let loaded = self.underlying_block_store.load(block_id).context(
            // Even if this fails due to corrupted data (i.e. decryption fails), we don't trigger an integrity violation because
            // the error message tells the user to fix integrity violations by removing the local KnownBlockVersions file,
            // but with a corrupted filesystem that wouldn't help.
            "IntegrityBlockStore failed to load the block from the underlying block store",
        )?;
        match loaded {
            None => Ok(None),
            Some(loaded) => {
                let data = self._check_and_remove_header(loaded, *block_id)?;
                Ok(Some(data))
            }
        }
    }

    fn num_blocks(&self) -> Result<u64> {
        self.underlying_block_store.num_blocks()
    }

    fn estimate_num_free_bytes(&self) -> Result<Byte> {
        self.underlying_block_store.estimate_num_free_bytes()
    }

    // TODO Test this by creating a blockstore based on an underlying block store (or on disk) and comparing the physical size. Same for encrypted block store or other stores with overhead.
    fn overhead(&self) -> Overhead {
        self.underlying_block_store.overhead()
            + Overhead::new(Byte::from_u64(u64::try_from(HEADER_SIZE_BYTES).unwrap()))
    }

    fn all_blocks(&self) -> Result<impl Iterator<Item = Result<BlockId>>> {
        self.underlying_block_store.all_blocks()
    }
}

impl<B> IntegrityBlockStore<B> {
    fn _check_and_remove_header(&self, data: Data, expected_block_id: BlockId) -> Result<Data> {
        ensure!(
            data.len() >= block_layout::data::OFFSET,
            "Block size is {} but we need at least {} to store the block header",
            data.len(),
            block_layout::data::OFFSET
        );
        let view = block_layout::View::new(data);
        trace!(
            "== Integrity data ({}B of overhead) ==",
            block_layout::data::OFFSET
        );
        trace!(
            "Format version header: {} ({}B)",
            view.format_version_header().read(),
            block_layout::format_version_header::SIZE.unwrap()
        );
        trace!(
            "Block id: {} ({}B)",
            hex::encode(view.block_id()),
            block_layout::block_id::SIZE.unwrap()
        );
        trace!(
            "Last update client id: {:?} ({}B)",
            view.last_update_client_id().try_read()?,
            block_layout::last_update_client_id::SIZE.unwrap()
        );
        trace!(
            "Block version: {} ({}B)",
            view.block_version().read(),
            block_layout::block_version::SIZE.unwrap()
        );

        Self::_check_format_version_header(&view)?;
        self._check_id_header(&view, &expected_block_id)?;

        // TODO Use view.into_data().extract(), but that requires adding an IntoSubregion trait to binary-layout that we can implement for our Data class.
        let mut data = view.into_storage();
        data.shrink_to_subregion(block_layout::data::OFFSET..);

        Ok(data)
    }

    fn _check_format_version_header(view: &block_layout::View<Data>) -> Result<()> {
        let format_version_header = view.format_version_header().read();
        if format_version_header != FORMAT_VERSION_HEADER {
            bail!(
                "Wrong FORMAT_VERSION_HEADER of {:?}. Expected {:?}. Maybe it was created with a different major version of CryFS?",
                format_version_header,
                FORMAT_VERSION_HEADER
            );
        }
        Ok(())
    }

    fn _check_id_header(
        &self,
        view: &block_layout::View<Data>,
        expected_block_id: &BlockId,
    ) -> Result<()> {
        let block_id = BlockId::from_array(view.block_id());
        if block_id != *expected_block_id {
            Err(IntegrityViolationError::WrongBlockId {
                id_from_filename: *expected_block_id,
                id_from_header: block_id,
            })?;
        }
        Ok(())
    }
}

impl<B: LLBlockStore> LLBlockStore for IntegrityBlockStore<B> {}
