use anyhow::{Context, Error, Result, anyhow, bail};
use base64::engine::{Engine as _, general_purpose::STANDARD as base64_STANDARD};
use byte_unit::Byte;
use log::trace;
use std::fmt::{self, Debug};
use std::fs::{DirEntry, ReadDir};
use std::path::{Path, PathBuf};

use super::sysinfo;
use super::{
    BLOCKID_LEN, BlockId, Overhead,
    interface::{BlockStoreReader, LLBlockStore},
};
use cryfs_utils::{data::Data, path::path_join};

// TODO Check if tokio-uring o uring-fs is faster than tokio::fs

const FORMAT_VERSION_HEADER_PREFIX: &[u8] = b"cryfs;block;";
const FORMAT_VERSION_HEADER: &[u8] = b"cryfs;block;0\0";

const PREFIX_LEN: usize = 3;
const NONPREFIX_LEN: usize = 2 * BLOCKID_LEN - PREFIX_LEN;

pub struct OnDiskBlockStore {
    basedir: PathBuf,
}

impl OnDiskBlockStore {
    pub fn new(basedir: PathBuf) -> Self {
        // TODO When we're creating a new file system, we should make sure that all the subfolders exist.
        //      Because we don't remove the folders when blocks get removed, otherwise a used file system looks different from a new one.
        Self { basedir }
    }
}

impl BlockStoreReader for OnDiskBlockStore {
    fn exists(&self, id: &BlockId) -> Result<bool> {
        let path = self._block_path(id);
        path_exists(&path)
    }

    fn load(&self, id: &BlockId) -> Result<Option<Data>> {
        let path = self._block_path(id);
        match std::fs::read(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist. Return None. This is not an error.
                Ok(None)
            }
            Err(err) => {
                Err(err).with_context(|| format!("Failed to open block file at {}", path.display()))
            }
            Ok(file_content) => {
                let block_content =
                    _check_and_remove_header(file_content.into()).with_context(|| {
                        format!(
                            "Failed to parse file contents of block at {}",
                            path.display()
                        )
                    })?;
                Ok(Some(block_content))
            }
        }
    }

    fn num_blocks(&self) -> Result<u64> {
        Ok(self
            ._all_block_files()
            .map(|blocks| blocks.flatten().count() as u64)?)
    }

    fn estimate_num_free_bytes(&self) -> Result<Byte> {
        sysinfo::get_available_disk_space(&self.basedir).map(Byte::from_u64)
    }

    fn overhead(&self) -> Overhead {
        Overhead::new(Byte::from_u64(FORMAT_VERSION_HEADER.len() as u64))
    }

    fn all_blocks(&self) -> Result<impl Iterator<Item = Result<BlockId>>> {
        self._all_block_files()
            .map(|iter| iter.map(|entry| entry.map(|entry| _blockid_from_filepath(&entry.path()))))
    }
}

fn path_exists(path: &Path) -> Result<bool> {
    let exists = std::fs::exists(path)?;
    Ok(exists)
}

impl Debug for OnDiskBlockStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OnDiskBlockStore")
    }
}

impl LLBlockStore for OnDiskBlockStore {}

impl OnDiskBlockStore {
    fn _block_path(&self, block_id: &BlockId) -> PathBuf {
        _block_path(self.basedir.as_path(), block_id)
    }

    fn _all_block_files(&self) -> Result<Box<dyn Iterator<Item = Result<DirEntry>>>> {
        Ok(Box::new(
            std::fs::read_dir(&self.basedir)
                .map_err(Error::from)?
                .flatten()
                .map(|subdir| -> Result<Option<ReadDir>, anyhow::Error> {
                    if subdir.metadata()?.is_dir()
                        && subdir.file_name().len() == PREFIX_LEN
                        && subdir
                            .file_name()
                            .to_str()
                            .ok_or_else(|| {
                                anyhow!(
                                    "Invalid UTF-8 in path in base directory: {}",
                                    subdir.path().display()
                                )
                            })?
                            .chars()
                            .all(_is_allowed_blockid_character)
                    {
                        // Return a stream with all the entries in the subdirectory
                        let entries_stream =
                            std::fs::read_dir(subdir.path()).map_err(Error::from)?;
                        Ok(Some(entries_stream))
                    } else {
                        // Dir entry is not a subdirectory with blocks, skip it.
                        Ok(None)
                    }
                })
                .flatten()
                .flatten()
                .flat_map(|b| b.into_iter())
                .flatten()
                .filter_map(|blockfile: DirEntry| -> Option<Result<DirEntry>> {
                    if blockfile.file_name().len() == NONPREFIX_LEN
                        && let Some(file_name) = blockfile.file_name().to_str()
                        && file_name.chars().all(_is_allowed_blockid_character)
                    {
                        return Some(Ok(blockfile));
                    } else {
                        // File doesn't match the blockfile pattern, skip it.
                        None
                    }
                }),
        ))
    }
}

// This function panics if the path doesn't match the correct pattern.
// Only call this on paths you know are matching the pattern!
fn _blockid_from_filepath(path: &Path) -> BlockId {
    let blockid_prefix = path
        .parent()
        .unwrap_or_else(|| {
            panic!(
                "Block file path `{}` should have a parent component",
                path.display()
            )
        })
        .file_name()
        .unwrap_or_else(|| {
            panic!(
                "Block file path `{}` should have a valid parent component",
                path.display()
            )
        })
        .to_str()
        .unwrap_or_else(|| panic!("Block file path `{}` should be valid UTF-8", path.display()));
    let blockid_nonprefix = path
        .file_name()
        .unwrap_or_else(|| {
            panic!(
                "Block file path `{}` should have a valid last component",
                path.display()
            )
        })
        .to_str()
        .unwrap_or_else(|| panic!("Block file path `{}` should be valid UTF-8", path.display()));
    assert_eq!(
        PREFIX_LEN,
        blockid_prefix.len(),
        "Block file path should have a prefix len of {} but was {:?}",
        PREFIX_LEN,
        blockid_prefix
    );
    assert_eq!(
        NONPREFIX_LEN,
        blockid_nonprefix.len(),
        "Block file path should have a nonprefix len of {} but was {:?}",
        NONPREFIX_LEN,
        blockid_nonprefix
    );
    let mut blockid = String::with_capacity(2 * BLOCKID_LEN);
    blockid.push_str(blockid_prefix);
    blockid.push_str(blockid_nonprefix);
    assert_eq!(2 * BLOCKID_LEN, blockid.len());
    BlockId::from_hex(&blockid)
        .with_context(|| format!("Block file path `{}` cannot be parsed as hex", blockid))
        .unwrap()
}

fn _check_and_remove_header(mut data: Data) -> Result<Data> {
    trace!(
        "== On disk data ({}B of overhead) ==",
        FORMAT_VERSION_HEADER.len()
    );
    trace!(
        "Format version header: {} ({}B)",
        str::from_utf8(FORMAT_VERSION_HEADER)?,
        FORMAT_VERSION_HEADER.len()
    );
    if !data.starts_with(FORMAT_VERSION_HEADER) {
        if data.starts_with(FORMAT_VERSION_HEADER_PREFIX) {
            bail!(
                "This block is not supported yet. Maybe it was created with a newer version of CryFS? Block: {}",
                base64_STANDARD.encode(data)
            );
        } else {
            bail!(
                "This is not a valid block: {}",
                base64_STANDARD.encode(data)
            );
        }
    }
    data.shrink_to_subregion(FORMAT_VERSION_HEADER.len()..);
    Ok(data)
}

fn _is_allowed_blockid_character(c: char) -> bool {
    c.is_ascii_digit() || ('A'..='F').contains(&c)
}

fn _block_path(basedir: &Path, block_id: &BlockId) -> PathBuf {
    // TODO Switch to lower-case hex, but if a block isn't found, fall back to reading the upper-case hex file for backwards compatibility.
    let block_id_str = block_id.to_hex_upper();
    assert!(
        block_id_str.chars().all(_is_allowed_blockid_character),
        "Created invalid block_id_str"
    );
    path_join(&[
        basedir,
        Path::new(&block_id_str[..PREFIX_LEN]),
        Path::new(&block_id_str[PREFIX_LEN..]),
    ])
}
