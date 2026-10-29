use super::DataNode;
use super::layout::{FORMAT_VERSION_HEADER, NodeLayout, node};
use anyhow::{Result, ensure};
use binary_layout::Field;
use byte_unit::Byte;
use log::trace;
use std::fmt::Debug;

use crate::blockstore::{Block, BlockId, BlockStore};
use cryfs_utils::data::Data;

pub struct DataLeafNode<B: BlockStore> {
    block: B::Block,
}

impl<B: BlockStore> DataLeafNode<B> {
    pub fn new(block: B::Block, layout: &NodeLayout) -> Result<Self> {
        trace!("Block node type: Leaf (depth == 0)");
        assert!(
            layout.block_size.as_u64() > u64::try_from(node::data::OFFSET).unwrap(),
            "Block doesn't have enough space for header. This should have been checked before calling DataLeafNode::new"
        );

        let view = node::View::new(block.data());
        ensure!(
            view.format_version_header().read() == FORMAT_VERSION_HEADER,
            "Loaded a node with format version {} but the current version is {}",
            view.format_version_header().read(),
            FORMAT_VERSION_HEADER,
        );
        assert_eq!(
            0,
            view.depth().read(),
            "Loaded a leaf with depth {}. This doesn't make sense, it should have been loaded as an inner node",
            view.depth().read(),
        );
        ensure!(
            Byte::from_u64(u64::try_from(block.data().len()).unwrap()) == layout.block_size,
            "Loaded block of size {} but expected {}",
            block.data().len(),
            layout.block_size,
        );
        trace!(
            "== Leaf node data (max {}B available in block)",
            layout.max_bytes_per_leaf()
        );
        let max_bytes_per_leaf = layout.max_bytes_per_leaf();
        let size = view.size().read();
        ensure!(
            size <= max_bytes_per_leaf,
            "Loaded a leaf that claims to store {} bytes but the maximum is {}.",
            size,
            max_bytes_per_leaf,
        );
        Ok(Self { block })
    }

    pub fn block_id(&self) -> &BlockId {
        self.block.block_id()
    }

    pub(super) fn raw_blockdata(&self) -> &Data {
        self.block.data()
    }

    pub(super) fn into_block(self) -> B::Block {
        self.block
    }

    pub fn num_bytes(&self) -> u32 {
        let view = node::View::new(self.block.data());
        view.size().read()
    }

    pub fn max_bytes_per_leaf(&self) -> u32 {
        NodeLayout {
            block_size: Byte::from_u64(u64::try_from(self.block.data().len()).unwrap()),
        }
        .max_bytes_per_leaf()
    }

    pub fn data(&self) -> &[u8] {
        let view = node::View::new(self.block.data().as_ref());
        let leaf_size = view.size().read() as usize;
        let leaf_data = view.into_data().into_slice();
        &leaf_data[..leaf_size]
    }

    pub fn upcast(self) -> DataNode<B> {
        DataNode::Leaf(self)
    }
}

impl<B: BlockStore> Debug for DataLeafNode<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataLeafNode")
            .field("block_id", &self.block_id())
            .field("num_bytes", &self.num_bytes())
            .finish()
    }
}
