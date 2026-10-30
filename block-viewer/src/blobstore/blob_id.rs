use anyhow::Result;
use binrw::{BinRead, BinWrite};

use crate::blockstore::{BLOCKID_LEN, BlockId};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, BinRead, BinWrite)]
pub struct BlobId {
    // TODO Remove `pub(super)` and use `Self::to_root_block_id()` instead.
    pub root: BlockId,
}

impl BlobId {
    pub fn new_random() -> Self {
        Self {
            root: BlockId::new_random(),
        }
    }

    pub const fn zero() -> Self {
        Self {
            root: BlockId::zero(),
        }
    }

    #[inline]
    pub fn to_root_block_id(&self) -> &BlockId {
        &self.root
    }

    #[inline]
    pub fn from_root_block_id(root: BlockId) -> Self {
        Self { root }
    }

    #[inline]
    pub fn from_slice(id_data: &[u8]) -> Result<Self> {
        Ok(Self {
            root: BlockId::from_slice(id_data)?,
        })
    }

    #[inline]
    pub fn from_array(id: &[u8; BLOCKID_LEN]) -> Self {
        Self {
            root: BlockId::from_array(id),
        }
    }

    #[inline]
    pub fn data(&self) -> &[u8; BLOCKID_LEN] {
        self.root.data()
    }

    pub fn from_hex(hex_data: &str) -> Result<Self> {
        Ok(Self {
            root: BlockId::from_hex(hex_data)?,
        })
    }

    pub fn to_hex(&self) -> String {
        self.root.to_hex()
    }
}

impl std::fmt::Display for BlobId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root)
    }
}

impl std::fmt::Debug for BlobId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlobId({self})")
    }
}
