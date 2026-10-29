use super::DataNode;
use super::layout::{NodeLayout, node};
use crate::blockstore::{BLOCKID_LEN, BlockId};
use anyhow::Result;
use binary_layout::Field;
use byte_unit::Byte;

use crate::blockstore::{BlockStore, InvalidBlockSizeError};

pub struct DataNodeStore<B: BlockStore> {
    block_store: B,
    layout: NodeLayout,
    physical_block_size: Byte,
}

impl<B: BlockStore> DataNodeStore<B> {
    pub fn new(block_store: B, physical_block_size: Byte) -> Result<Self, InvalidBlockSizeError> {
        let block_size = Self::_block_size(&block_store, physical_block_size)?;

        Ok(Self {
            block_store,
            layout: NodeLayout { block_size },
            physical_block_size,
        })
    }

    fn _block_size(
        block_store: &B,
        physical_block_size: Byte,
    ) -> Result<Byte, InvalidBlockSizeError> {
        let block_size = block_store
            .overhead()
            .usable_block_size_from_physical_block_size(physical_block_size)?;

        // Min block size: enough for header and for inner nodes to have at least two children and form a tree.
        let min_block_size =
            Byte::from_u64(u64::try_from(node::data::OFFSET + 2 * BLOCKID_LEN).unwrap());
        if block_size < min_block_size {
            Err(InvalidBlockSizeError::new(format!(
                "Tried to create a DataNodeStore with block size {block_size} (physical: {physical_block_size}) but must be at least {min_block_size}",
            )))
        } else {
            Ok(block_size)
        }
    }

    pub fn layout(&self) -> &NodeLayout {
        &self.layout
    }

    pub fn load(&self, block_id: BlockId) -> Result<Option<DataNode<B>>> {
        match self.block_store.load(block_id)? {
            None => Ok(None),
            Some(block) => DataNode::parse(block, &self.layout).map(Some),
        }
    }

    pub fn num_nodes(&self) -> Result<u64> {
        self.block_store.num_blocks()
    }

    pub fn estimate_space_for_num_blocks_left(&self) -> Result<u64> {
        Ok(
            self.block_store.estimate_num_free_bytes()?.as_u64()
                / self.physical_block_size.as_u64(),
        )
    }

    pub fn logical_block_size_bytes(&self) -> Byte {
        Byte::from_u64(u64::from(self.layout.max_bytes_per_leaf()))
    }
}
