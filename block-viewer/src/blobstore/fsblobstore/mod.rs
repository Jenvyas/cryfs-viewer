mod base_blob;
mod layout;
use byte_unit::Byte;
pub use layout::BlobType;

pub use base_blob::BaseBlob;

use anyhow::Result;

mod fstypes;

mod file_blob;
pub use file_blob::FileBlob;

mod dir_blob;
pub use dir_blob::{DIR_LSTAT_SIZE, DirBlob, MODE_NEW_SYMLINK};

mod symlink_blob;
pub use symlink_blob::SymlinkBlob;

mod dir_entries;

use crate::blobstore::{BlobId, BlobStore};
use crate::blockstore::BlockId;
// TODO Now that FileBlob, DirBlob and SymlinkBlob are only ever returned as references,
//      we can probably store BaseBlob directly in here and just have FileBlob, DirBlob and SymlinkBlob
//      store a reference to BaseBlob instead of owning BaseBlob.

#[derive(Debug)]
pub struct FsBlobStore<B: BlobStore> {
    blobstore: B,
}

impl<B: BlobStore> FsBlobStore<B> {
    pub fn new(blobstore: B) -> Self {
        Self { blobstore }
    }

    pub fn load(&self, blob_id: &BlobId) -> Result<Option<FsBlob<B>>> {
        if let Some(blob) = self.blobstore.load(blob_id)? {
            Ok(Some(FsBlob::parse(blob)?))
        } else {
            Ok(None)
        }
    }

    pub fn num_blocks(&self) -> Result<u64> {
        self.blobstore.num_nodes()
    }

    pub fn estimate_space_for_num_blocks_left(&self) -> Result<u64> {
        self.blobstore.estimate_space_for_num_blocks_left()
    }

    // logical means "space we can use" as opposed to "space it takes on the disk" (i.e. logical is without headers, checksums, ...)
    pub fn logical_block_size_bytes(&self) -> Byte {
        self.blobstore.logical_block_size_bytes()
    }
}

pub enum FsBlob<B: BlobStore> {
    File(FileBlob<B>),
    Directory(DirBlob<B>),
    Symlink(SymlinkBlob<B>),
}

impl<B: BlobStore> FsBlob<B> {
    pub fn parse(blob: B::ConcreteBlob) -> Result<FsBlob<B>> {
        let blob = BaseBlob::parse(blob)?;
        let blob_type = blob.blob_type()?;
        match blob_type {
            BlobType::Dir => Ok(Self::Directory(DirBlob::new(blob)?)),
            BlobType::File => Ok(Self::File(FileBlob::new(blob))),
            BlobType::Symlink => Ok(Self::Symlink(SymlinkBlob::new(blob))),
        }
    }

    pub fn blob_id(&self) -> BlobId {
        match &self {
            Self::File(blob) => blob.blob_id(),
            Self::Directory(blob) => blob.blob_id(),
            Self::Symlink(blob) => blob.blob_id(),
        }
    }

    pub fn parent(&self) -> BlobId {
        match &self {
            Self::File(blob) => blob.parent(),
            Self::Directory(blob) => blob.parent(),
            Self::Symlink(blob) => blob.parent(),
        }
    }

    pub fn blob_type(&self) -> BlobType {
        match self {
            Self::File(_) => BlobType::File,
            Self::Directory(_) => BlobType::Dir,
            Self::Symlink(_) => BlobType::Symlink,
        }
    }

    pub fn lstat_size(&mut self) -> Result<u64> {
        match self {
            Self::File(blob) => blob.lstat_size(),
            Self::Directory(blob) => Ok(blob.lstat_size()),
            Self::Symlink(blob) => blob.lstat_size(),
        }
    }

    pub fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>> {
        match self {
            Self::File(blob) => blob.all_blocks(),
            Self::Directory(blob) => blob.all_blocks(),
            Self::Symlink(blob) => blob.all_blocks(),
        }
    }
}
