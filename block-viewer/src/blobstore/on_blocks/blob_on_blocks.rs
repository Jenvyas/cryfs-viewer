use std::ops::Div;
use std::sync::Arc;

use super::super::DataNodeStore;
use crate::blobstore::data_node::DataNode;
use crate::blobstore::{Blob, BlobId};
use crate::blockstore::{BlockId, BlockStore};
use anyhow::Result;
use cryfs_utils::data::Data;

pub struct BlobOnBlocks<B: BlockStore> {
    pub(super) root_blob: DataNode<B>,
    pub(super) node_store: Arc<DataNodeStore<B>>,
}

impl<B: BlockStore> Blob for BlobOnBlocks<B> {
    fn id(&self) -> BlobId {
        BlobId {
            root: *self.root_blob.block_id(),
        }
    }

    fn num_bytes(&mut self) -> Result<u64> {
        Ok(match &self.root_blob {
            DataNode::Inner(node) => {
                // TODO: We're currently traversing in such a way that we only look at nodes 1 layer deep
                // and assume that they're leaf nodes we should keep traversing until we reach actual leaf nodes
                let (num_full_children, last_node) = node.children().enumerate().last().unwrap();
                let last_node_size = self
                    .node_store
                    .load(last_node)?
                    .ok_or(anyhow::Error::msg("Child doesn't exist"))?
                    .into_leaf_node()
                    .ok_or(anyhow::Error::msg("Child isn't a leaf node."))?
                    .num_bytes() as u64;
                let total_size = (num_full_children as u64)
                    * (self.node_store.layout().max_bytes_per_leaf() as u64)
                    + last_node_size;
                total_size
            }
            DataNode::Leaf(node) => node.num_bytes() as u64,
        })
    }

    fn read_all(&mut self) -> Result<cryfs_utils::data::Data> {
        let mut data = Data::from(vec![0; self.num_bytes()? as usize]);
        self.read(&mut data, 0)?;
        Ok(data)
    }

    fn read(&mut self, target: &mut [u8], offset: u64) -> Result<()> {
        match &self.root_blob {
            DataNode::Inner(node) => {
                let children_ids: Vec<_> = node.children().collect();
                let last_node_bytes = self
                    .node_store
                    .load(*children_ids.last().unwrap())?
                    .ok_or(anyhow::Error::msg("Child doesn't exist"))?
                    .into_leaf_node()
                    .ok_or(anyhow::Error::msg("Child isn't a leaf node."))?
                    .num_bytes() as u64;

                let full_children = (children_ids.len() - 1) as u64;
                let block_size = self.node_store.logical_block_size_bytes().as_u64();
                let total_size = full_children * block_size + last_node_bytes;

                if offset >= total_size {
                    Err(anyhow::Error::msg("Offset out of bounds"))?;
                }

                let possible_to_read = target.len().min((total_size - offset) as usize);
                let node_offset = offset.div(block_size) as usize;
                let first_block_offset = (offset % block_size) as usize;
                // Up to what byte it's possible to read from the first block.
                let first_block_read_bytes =
                    (block_size as usize).min(first_block_offset + possible_to_read);
                let remainding_bytes =
                    possible_to_read.saturating_sub(block_size as usize - first_block_offset);
                // number of full blocks to be read into target
                let full_blocks = remainding_bytes / block_size as usize;
                let last_block_bytes = remainding_bytes % block_size as usize;

                let first_target = self
                    .node_store
                    .load(children_ids[node_offset])?
                    .ok_or(anyhow::Error::msg("Child doesn't exist"))?
                    .into_leaf_node()
                    .ok_or(anyhow::Error::msg("Child isn't a leaf node."))?;

                let mut current_offset = first_block_read_bytes;
                target[..current_offset].clone_from_slice(
                    &first_target.data()[first_block_offset..first_block_read_bytes],
                );

                if full_blocks > 0 {
                    for child in
                        (&children_ids[(node_offset + 1)..(node_offset + 1 + full_blocks)]).iter()
                    {
                        // TODO: We're currently traversing in such a way that we only look at nodes 1 layer deep
                        // and assume that they're leaf nodes we should keep traversing until we reach actual leaf nodes
                        let child = self
                            .node_store
                            .load(*child)?
                            .ok_or(anyhow::Error::msg("Child doesn't exist"))?;
                        let leaf = match child {
                            DataNode::Inner(_) => todo!("Implement proper traversal."),
                            DataNode::Leaf(node) => node,
                        };

                        let read_until = current_offset + block_size as usize;
                        target[current_offset..read_until].copy_from_slice(leaf.data());
                        current_offset += read_until;
                    }
                }

                if last_block_bytes > 0 {
                    let last_target = self
                        .node_store
                        .load(children_ids[node_offset + full_blocks + 1])?
                        .ok_or(anyhow::Error::msg("Child doesn't exist"))?
                        .into_leaf_node()
                        .ok_or(anyhow::Error::msg("Child isn't a leaf node."))?;
                    let data = last_target.data();
                    target[current_offset..(current_offset + last_block_bytes)]
                        .copy_from_slice(&data[..last_block_bytes]);
                }
            }
            DataNode::Leaf(node) => {
                if offset >= node.num_bytes() as u64 {
                    Err(anyhow::Error::msg("Offset is out of bounds of leaf node"))?;
                }
                if offset + target.len() as u64 > node.num_bytes() as u64 {
                    Err(anyhow::Error::msg(
                        "Offset + target is out of bounds of leaf node",
                    ))?;
                }
                let offset = offset as usize;
                target.copy_from_slice(&node.data()[offset..(target.len() + offset)]);
                node.data();
            }
        }
        Ok(())
    }

    fn try_read(&mut self, target: &mut [u8], offset: u64) -> Result<usize> {
        todo!()
    }

    fn num_nodes(&mut self) -> Result<u64> {
        todo!()
    }

    fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>> {
        let ids = match &self.root_blob {
            DataNode::Inner(inner) => inner
                .children()
                .map(|child| Ok(child))
                .collect::<Vec<Result<BlockId>>>(),
            DataNode::Leaf(data_leaf_node) => vec![Ok(data_leaf_node.block_id().clone())],
        };
        Ok(Box::new(ids.into_iter()))
    }
}
