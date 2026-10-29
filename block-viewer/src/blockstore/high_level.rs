use anyhow::Result;
use byte_unit::Byte;

use super::{BlockId, Overhead};
use cryfs_utils::data::Data;

// High level block interface f

// TODO Now that we have the interface, go through downstream code and see where we can replace direct use of LockingBlockStore with the BlockStore trait.

pub trait Block {
    fn block_id(&self) -> &BlockId;
    fn data(&self) -> &Data;
}

pub trait BlockStore {
    type Block: Block;

    fn load(&self, block_id: BlockId) -> Result<Option<Self::Block>>;

    // Note: for any blocks that are created or removed while the returned stream is running,
    // we don't give any guarantees for whether they're counted or not.
    fn num_blocks(&self) -> Result<u64>;
    fn estimate_num_free_bytes(&self) -> Result<Byte>;
    fn overhead(&self) -> Overhead;

    // Note: for any blocks that are created or removed while the returned stream is running,
    // we don't give any guarantees for whether they'll be part of the stream or not.
    fn all_blocks(&self) -> Result<impl Iterator<Item = Result<BlockId>>>;
}
