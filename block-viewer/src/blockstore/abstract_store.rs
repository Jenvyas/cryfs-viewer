use anyhow::Result;
use cryfs_blockstore::BlockId;
use cryfs_utils::data::Data;

use crate::blockstore::interface::LLBlockStore;

pub struct AbstractBlockStore<B: super::interface::LLBlockStore> {
    base_store: B,
}

impl<B: super::interface::LLBlockStore> AbstractBlockStore<B> {
    pub fn new(base_store: B) -> Self {
        Self { base_store }
    }
}

impl<B: LLBlockStore> super::high_level::BlockStore for AbstractBlockStore<B> {
    type Block = AbstractBlock;

    fn load(&self, block_id: BlockId) -> Result<Option<Self::Block>> {
        self.base_store
            .load(&block_id)
            .map(|loaded| loaded.map(|data| AbstractBlock { data, block_id }))
    }

    fn num_blocks(&self) -> anyhow::Result<u64> {
        self.base_store.num_blocks()
    }

    fn estimate_num_free_bytes(&self) -> anyhow::Result<byte_unit::Byte> {
        self.base_store.estimate_num_free_bytes()
    }

    fn overhead(&self) -> super::Overhead {
        self.base_store.overhead()
    }

    fn all_blocks(&self) -> anyhow::Result<impl Iterator<Item = Result<BlockId>>> {
        self.base_store.all_blocks()
    }
}

pub struct AbstractBlock {
    block_id: BlockId,
    data: Data,
}

impl super::high_level::Block for AbstractBlock {
    fn block_id(&self) -> &BlockId {
        &self.block_id
    }

    fn data(&self) -> &cryfs_utils::data::Data {
        &self.data
    }
}
