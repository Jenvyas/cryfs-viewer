use anyhow::{Result, anyhow, ensure};
use binary_layout::Field;
use log::trace;

use super::super::{Blob, BlobId, BlobStore};
use super::layout::{self, FORMAT_VERSION_HEADER};
use crate::blockstore::BlockId;
use cryfs_utils::data::Data;

pub struct BaseBlob<B>
where
    B: BlobStore,
{
    blob: B::ConcreteBlob,
    header_cache: layout::fsblob_header::View<Data>,
}

impl<B: BlobStore> BaseBlob<B> {
    pub fn parse(mut blob: B::ConcreteBlob) -> Result<BaseBlob<B>> {
        // TODO No need to zero-initialize
        trace!("== Filesystem blob data ==");
        let mut header = vec![0; layout::fsblob_header::SIZE.unwrap()];
        blob.read(&mut header, 0)?;
        let header_cache = layout::fsblob_header::View::new(header.into());
        trace!(
            "Format version header: {} ({}B)",
            header_cache.format_version_header().read(),
            layout::fsblob_header::format_version_header::SIZE.unwrap(),
        );
        trace!(
            "Filesystem blob type: {:?} ({}B)",
            header_cache.blob_type().try_read()?,
            layout::fsblob_header::blob_type::SIZE.unwrap()
        );
        trace!(
            "Filesystem blob parent: {} ({}B)",
            BlockId::from_array(header_cache.parent()),
            layout::fsblob_header::parent::SIZE.unwrap()
        );
        let format_version = header_cache.format_version_header().read();
        let version_check = if format_version == FORMAT_VERSION_HEADER {
            Ok(())
        } else {
            Err(anyhow!(
                "Loaded FsBlob with format version {} but current version is {}",
                format_version,
                FORMAT_VERSION_HEADER
            ))
        };
        Ok(Self { blob, header_cache })
    }

    pub fn blob_id(&self) -> BlobId {
        self.blob.id()
    }

    pub fn blob_type(&self) -> Result<layout::BlobType> {
        Ok(self.header_cache.blob_type().try_read()?)
    }

    pub fn parent(&self) -> BlobId {
        BlobId::from_array(self.header_cache.parent())
    }

    pub fn num_data_bytes(&mut self) -> Result<u64> {
        // TODO Make self parameter non-mut?
        Ok(self.blob.num_bytes()? - layout::fsblob_header::SIZE.unwrap() as u64)
    }

    pub fn try_read_data(&mut self, target: &mut [u8], offset: u64) -> Result<usize> {
        // TODO Make self parameter non-mut?
        self.blob
            .try_read(target, offset + layout::fsblob_header::SIZE.unwrap() as u64)
    }

    pub fn read_all_data(&mut self) -> Result<Data> {
        // TODO We should probably enforce a max size for the read so we don't block when a file system is bad
        //      This is only used for symlink blobs right now and those aren't supposed to be that large.
        let mut data = self.blob.read_all()?;
        ensure!(
            data.len() >= layout::fsblob_header::SIZE.unwrap(),
            "Blob is too small to contain a header"
        );
        data.shrink_to_subregion(layout::fsblob_header::SIZE.unwrap()..);
        Ok(data)
    }

    pub fn all_blocks(&self) -> Result<Box<dyn Iterator<Item = Result<BlockId>>>> {
        self.blob.all_blocks()
    }
}
