use anyhow::Result;

use super::base_blob::BaseBlob;
use super::fstypes::Mode;
use crate::blobstore::{BlobId, BlobStore};
use crate::blockstore::BlockId;
use cryfs_utils::path::PathComponent;

use super::dir_entries::{DirEntry, DirEntryList};

pub const DIR_LSTAT_SIZE: u64 = 4096;
pub const MODE_NEW_SYMLINK: Mode = Mode::zero()
    .with_symlink_flag()
    .with_user_read_flag()
    .with_user_write_flag()
    .with_user_exec_flag()
    .with_group_read_flag()
    .with_group_write_flag()
    .with_group_exec_flag()
    .with_other_read_flag()
    .with_other_write_flag()
    .with_other_exec_flag();

pub struct DirBlob<B: BlobStore> {
    blob: BaseBlob<B>,
    entries: DirEntryList,
}

impl<B: BlobStore> DirBlob<B> {
    // TODO Some of the functions in here (and possibly in other blobs) were only needed for the cxx.rs bindings. Check which ones we can delete now since we're rust only.

    pub(super) fn new(mut blob: BaseBlob<B>) -> Result<DirBlob<B>> {
        let entries = DirEntryList::deserialize(&mut blob)?;
        Ok(Self { blob, entries })
    }

    // TODO DoubleEndedIterator + FusedIterator
    pub fn entries(&self) -> impl Iterator<Item = &DirEntry> + ExactSizeIterator + use<'_, B> {
        self.entries.iter()
    }

    pub fn blob_id(&self) -> BlobId {
        self.blob.blob_id()
    }

    pub fn parent(&self) -> BlobId {
        self.blob.parent()
    }

    pub fn num_entries(&self) -> usize {
        self.entries.num_entries()
    }

    pub fn entry_by_id(&self, id: &BlobId) -> Option<&DirEntry> {
        self.entries.get_by_id(id)
    }

    pub fn entry_by_name(&self, name: &PathComponent) -> Option<&DirEntry> {
        self.entries.get_by_name(name)
    }

    pub fn lstat_size(&self) -> u64 {
        DIR_LSTAT_SIZE
    }

    pub fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>> {
        // TODO We may want to flush here since otherwise any change aren't written yet
        self.blob.all_blocks()
    }
}
