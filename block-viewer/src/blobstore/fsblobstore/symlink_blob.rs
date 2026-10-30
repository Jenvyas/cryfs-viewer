use anyhow::Result;

use super::base_blob::BaseBlob;
use crate::blobstore::{BlobId, BlobStore};
use crate::blockstore::BlockId;

pub struct SymlinkBlob<B: BlobStore> {
    blob: BaseBlob<B>,
}

impl<B: BlobStore> SymlinkBlob<B> {
    pub(super) fn new(blob: BaseBlob<B>) -> Self {
        Self { blob }
    }

    pub fn blob_id(&self) -> BlobId {
        self.blob.blob_id()
    }

    pub fn target(&mut self) -> Result<String> {
        // TODO If blob.read_all() took &self instead of &mut self, we wouldn't have to make self "mut" in the parameter list above
        // TODO Should we cache the target and only read it once?
        let data = self.blob.read_all_data()?;
        let target = String::from_utf8(data.into_vec())?;
        Ok(target)
    }

    pub fn parent(&self) -> BlobId {
        self.blob.parent()
    }

    pub fn lstat_size(&mut self) -> Result<u64> {
        Ok(self.target()?.len() as u64)
    }

    pub fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>> {
        self.blob.all_blocks()
    }
}
