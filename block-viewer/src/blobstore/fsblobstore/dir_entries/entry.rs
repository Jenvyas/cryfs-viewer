use anyhow::Result;
use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroU8;
use std::time::SystemTime;

use super::super::fstypes::{Gid, Mode, Uid};
use crate::blobstore::BlobId;
use cryfs_utils::{
    binary::{read_null_string, read_timespec, write_null_string, write_timespec},
    path::{PathComponent, PathComponentBuf},
};

// TODO Unify this with the BlobType enum from this very same crate?

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little, repr=u8)]
pub enum EntryType {
    Dir = 0x00,
    File = 0x01,
    Symlink = 0x02,
}

#[derive(Clone, BinRead, BinWrite, Debug)]
#[brw(little)]
pub struct DirEntryImpl {
    entry_type: EntryType,
    mode: Mode,
    uid: Uid,
    gid: Gid,
    #[br(parse_with = read_timespec)]
    #[bw(write_with = write_timespec)]
    last_access_time: SystemTime,
    #[br(parse_with = read_timespec)]
    #[bw(write_with = write_timespec)]
    last_modification_time: SystemTime,
    #[br(parse_with = read_timespec)]
    #[bw(write_with = write_timespec)]
    last_metadata_change_time: SystemTime,
    #[br(parse_with = read_path_component)]
    #[bw(write_with = write_path_component)]
    name: PathComponentBuf,
    blob_id: BlobId,
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    // Add an indirection so that outside code can't directly serialize/deserialize it
    // without going through our validation logic.
    inner: DirEntryImpl,
}

impl DirEntry {
    pub fn new(
        entry_type: EntryType,
        name: PathComponentBuf,
        blob_id: BlobId,
        mode: Mode,
        uid: Uid,
        gid: Gid,
        last_access_time: SystemTime,
        last_modification_time: SystemTime,
        last_metadata_change_time: SystemTime,
    ) -> Self {
        let mode = match entry_type {
            EntryType::File => mode.with_file_flag(),
            EntryType::Dir => mode.with_dir_flag(),
            EntryType::Symlink => mode.with_symlink_flag(),
        };
        Self {
            inner: DirEntryImpl {
                entry_type,
                mode,
                uid,
                gid,
                last_access_time,
                last_modification_time,
                last_metadata_change_time,
                name,
                blob_id,
            },
        }
    }

    pub fn deserialize(source: &mut (impl Read + Seek)) -> Result<Self> {
        let result = Self {
            inner: DirEntryImpl::read(source)?,
        };
        Ok(result)
    }

    pub fn entry_type(&self) -> EntryType {
        self.inner.entry_type
    }

    pub fn mode(&self) -> Mode {
        self.inner.mode
    }

    pub fn uid(&self) -> Uid {
        self.inner.uid
    }

    pub fn gid(&self) -> Gid {
        self.inner.gid
    }

    pub fn last_access_time(&self) -> SystemTime {
        self.inner.last_access_time
    }

    pub fn last_modification_time(&self) -> SystemTime {
        self.inner.last_modification_time
    }

    pub fn last_metadata_change_time(&self) -> SystemTime {
        self.inner.last_metadata_change_time
    }

    pub fn name(&self) -> &PathComponent {
        &self.inner.name
    }

    pub fn blob_id(&self) -> &BlobId {
        &self.inner.blob_id
    }
}

// TODO Tests
fn read_path_component<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<PathComponentBuf> {
    // TODO Direct implementation without going through an intermediate Vec<NonZeroU8> would be faster
    let pos = reader.seek(SeekFrom::Current(0))?;
    let read_str = read_null_string(reader, endian, ())?;
    let read_str = read_str.into_iter().map(|v| v.get()).collect();
    let read_str = String::from_utf8(read_str).map_err(|err| binrw::Error::AssertFail {
        pos,
        message: format!("{err:?}"),
    })?;
    let path =
        PathComponentBuf::try_from_string(read_str).map_err(|err| binrw::Error::AssertFail {
            pos,
            message: format!("{err:?}"),
        })?;
    Ok(path)
}

// TODO Tests
fn write_path_component(
    v: &PathComponentBuf,
    writer: &mut (impl Write + Seek),
    endian: Endian,
    args: (),
) -> Result<(), binrw::Error> {
    // TODO Direct implementation without going through an intermediate Vec<NonZeroU8> would be faster
    let bytes: Vec<NonZeroU8> = v
        .as_str()
        .as_bytes()
        .into_iter()
        .map(|c| {
            NonZeroU8::try_from(*c).expect("PathComponent ensures that there aren't any null bytes")
        })
        .collect();
    write_null_string(&bytes, writer, endian, args)
}
