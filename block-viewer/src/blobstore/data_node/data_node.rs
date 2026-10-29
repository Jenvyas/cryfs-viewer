use super::DataInnerNode;
use super::DataLeafNode;
use super::layout::{FORMAT_VERSION_HEADER, NodeLayout, node};
use crate::blockstore::{Block, BlockId, BlockStore};
use anyhow::{Result, ensure};
use binary_layout::Field;
use cryfs_utils::data::Data;
use log::trace;

#[derive(Debug)]
pub enum DataNode<B: BlockStore> {
    Inner(DataInnerNode<B>),
    Leaf(DataLeafNode<B>),
}

impl<B: BlockStore> DataNode<B> {
    pub fn parse(block: B::Block, layout: &NodeLayout) -> Result<Self> {
        ensure!(
            usize::try_from(layout.block_size).unwrap() == block.data().len(),
            "Expected to load block of size {} but loaded block {:?} had size {}",
            layout.block_size,
            block.block_id(),
            block.data().len(),
        );
        trace!("== Node data ({}B of overhead)  ==", node::data::OFFSET);

        let node_view = node::View::new(block.data());
        let format_version_header = node_view.format_version_header().read();

        trace!(
            "Format version header: {} ({}B)",
            node_view.format_version_header().read(),
            node::format_version_header::SIZE.unwrap()
        );
        ensure!(
            FORMAT_VERSION_HEADER == format_version_header,
            "Loaded a node {:?} with format_version_header == {}. This is not a supported format.",
            block.block_id(),
            format_version_header
        );
        trace!(
            "Unused node data for alignment: {}B",
            node::unused::SIZE.unwrap(),
        );
        trace!(
            "Node depth: {} ({}B)",
            node_view.depth().read(),
            node::depth::SIZE.unwrap()
        );
        trace!(
            "Node size (stored bytes for Leaf, number of children for Inner): {} ({}B)",
            node_view.size().read(),
            node::size::SIZE.unwrap()
        );
        let depth = node_view.depth().read();
        if depth == 0 {
            Ok(DataNode::Leaf(DataLeafNode::new(block, layout)?))
        } else {
            Ok(DataNode::Inner(DataInnerNode::new(block, layout)?))
        }
    }

    pub fn depth(&self) -> u8 {
        match self {
            Self::Leaf(_) => 0,
            Self::Inner(inner) => inner.depth().get(),
        }
    }

    pub fn block_id(&self) -> &BlockId {
        match self {
            Self::Leaf(leaf) => leaf.block_id(),
            Self::Inner(inner) => inner.block_id(),
        }
    }

    // TODO No pub(crate) but rather pub(super::super)?
    pub(crate) fn raw_blockdata(&self) -> &Data {
        match self {
            Self::Leaf(leaf) => leaf.raw_blockdata(),
            Self::Inner(inner) => inner.raw_blockdata(),
        }
    }

    pub(crate) fn _into_block(self) -> B::Block {
        match self {
            Self::Leaf(leaf) => leaf.into_block(),
            Self::Inner(inner) => inner.into_block(),
        }
    }

    pub fn into_inner_node(self) -> Option<DataInnerNode<B>> {
        match self {
            Self::Inner(inner) => Some(inner),
            Self::Leaf(_) => None,
        }
    }

    pub fn into_leaf_node(self) -> Option<DataLeafNode<B>> {
        match self {
            Self::Inner(_) => None,
            Self::Leaf(leaf) => Some(leaf),
        }
    }
}
