use anyhow::Result;

use crate::blobstore::{BlobId, BlobStore};

use super::base_blob::BaseBlob;
use crate::blockstore::BlockId;

pub struct FileBlob<B: BlobStore> {
    blob: BaseBlob<B>,
}

impl<B: BlobStore> FileBlob<B> {
    pub(super) fn new(blob: BaseBlob<B>) -> Self {
        Self { blob }
    }

    pub fn blob_id(&self) -> BlobId {
        self.blob.blob_id()
    }

    pub fn num_bytes(&mut self) -> Result<u64> {
        self.blob.num_data_bytes()
    }

    pub fn try_read(&mut self, target: &mut [u8], offset: u64) -> Result<usize> {
        self.blob.try_read_data(target, offset)
    }

    pub fn parent(&self) -> BlobId {
        self.blob.parent()
    }

    pub fn lstat_size(&mut self) -> Result<u64> {
        self.num_bytes()
    }

    pub fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>> {
        self.blob.all_blocks()
    }
}
