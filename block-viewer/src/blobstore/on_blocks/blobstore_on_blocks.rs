use std::sync::Arc;

use anyhow::Result;
use byte_unit::Byte;

use crate::{
    blobstore::{BlobId, BlobStore, DataNodeStore, on_blocks::BlobOnBlocks},
    blockstore::{BlockStore, InvalidBlockSizeError},
};

pub struct BlobStoreOnBlocks<B: BlockStore> {
    node_store: Arc<DataNodeStore<B>>,
}

impl<B: BlockStore> BlobStoreOnBlocks<B> {
    pub fn new(blockstore: B, physical_block_size: Byte) -> Result<Self, InvalidBlockSizeError> {
        Ok(Self {
            node_store: Arc::new(DataNodeStore::new(blockstore, physical_block_size)?),
        })
    }
}

impl<B: BlockStore> BlobStore for BlobStoreOnBlocks<B> {
    type ConcreteBlob = BlobOnBlocks<B>;

    fn load(&self, id: &BlobId) -> Result<Option<Self::ConcreteBlob>> {
        self.node_store.load(id.root).map(|node| {
            node.map(|root_blob| BlobOnBlocks {
                root_blob,
                node_store: self.node_store.clone(),
            })
        })
    }

    fn num_nodes(&self) -> Result<u64> {
        self.node_store.num_nodes()
    }

    fn estimate_space_for_num_blocks_left(&self) -> Result<u64> {
        self.node_store.estimate_space_for_num_blocks_left()
    }

    fn logical_block_size_bytes(&self) -> Byte {
        self.node_store.logical_block_size_bytes()
    }
}
