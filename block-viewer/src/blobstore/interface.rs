use anyhow::Result;
use byte_unit::Byte;

use crate::blockstore::{BLOCKID_LEN, BlockId};
use cryfs_utils::data::Data;

use super::BlobId;

pub const BLOBID_LEN: usize = BLOCKID_LEN;

pub trait Blob: Sized {
    fn id(&self) -> BlobId;
    // TODO Can we make size take &self instead of &mut self? Same for other read-only functions?
    fn num_bytes(&mut self) -> Result<u64>;

    fn read_all(&mut self) -> Result<Data>;
    fn read(&mut self, target: &mut [u8], offset: u64) -> Result<()>;
    fn try_read(&mut self, target: &mut [u8], offset: u64) -> Result<usize>;
    // TODO `num_nodes` and `all_blocks` is a leaky abstraction because it gives away that we use blocks. Remove these.
    fn num_nodes(&mut self) -> Result<u64>;

    fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>>;
}

pub trait BlobStore {
    type ConcreteBlob: Blob;

    fn load(&self, id: &BlobId) -> Result<Option<Self::ConcreteBlob>>;
    fn num_nodes(&self) -> Result<u64>;
    fn estimate_space_for_num_blocks_left(&self) -> Result<u64>;
    // logical means "space we can use" as opposed to "space it takes on the disk" (i.e. logical is without headers, checksums, ...)
    fn logical_block_size_bytes(&self) -> Byte;
}
