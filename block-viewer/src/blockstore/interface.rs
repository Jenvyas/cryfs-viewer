use anyhow::Result;
use byte_unit::Byte;
use std::any::Any;

use super::{BlockId, Overhead};
use cryfs_utils::data::Data;

pub trait BlockStoreReader {
    // TODO Add test cases for exists(), they're not among the C++ test cases since we added it later
    fn exists(&self, id: &BlockId) -> Result<bool>;
    fn load(&self, id: &BlockId) -> Result<Option<Data>>;
    fn num_blocks(&self) -> Result<u64>;
    fn estimate_num_free_bytes(&self) -> Result<Byte>;
    fn overhead(&self) -> Overhead;

    fn all_blocks(&self) -> Result<impl Iterator<Item = Result<BlockId>>>;
}

pub trait LLBlockStore: BlockStoreReader + Any {}
