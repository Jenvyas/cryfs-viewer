use anyhow::Result;
use std::fmt::Debug;
use std::io::Cursor;

use super::super::base_blob::BaseBlob;
use super::entry::DirEntry;
use crate::blobstore::{BlobId, BlobStore};
use cryfs_utils::path::PathComponent;

#[derive(Debug)]
pub struct DirEntryList {
    // TODO The implementation currently assumes that there is at most one
    // entry with any given block id. If we add hard links, we have to change this.
    // TODO While we don't have hardlinks, add invariant assertions that each
    // id only exists once and that entries are ordered.
    entries: Vec<DirEntry>,
}

impl DirEntryList {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn get_by_name(&self, name: &PathComponent) -> Option<&DirEntry> {
        // TODO Instead of linear search, we could use a HashMap
        for entry in &self.entries {
            if entry.name() == name {
                return Some(entry);
            }
        }
        None
    }

    fn _get_by_name_with_index(&self, name: &PathComponent) -> Option<(usize, &DirEntry)> {
        // TODO Instead of linear search, we could use a HashMap
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.name() == name {
                return Some((index, entry));
            }
        }
        None
    }

    fn _get_index_by_name(&self, name: &PathComponent) -> Option<usize> {
        self._get_by_name_with_index(name).map(|(index, _)| index)
    }

    fn _get_index_by_id(&self, id: &BlobId) -> Option<usize> {
        let found = self._find_lower_bound(id);
        if found == self.entries.len() || self.entries[found].blob_id() != id {
            return None;
        }
        Some(found)
    }

    fn _find_lower_bound(&self, id: &BlobId) -> usize {
        self._find_first_index(id, |entry| entry.blob_id() >= id)
    }

    fn _find_upper_bound(&self, id: &BlobId) -> usize {
        self._find_first_index(id, |entry| entry.blob_id() > id)
    }

    fn _find_first_index(&self, hint: &BlobId, mut pred: impl FnMut(&DirEntry) -> bool) -> usize {
        // TODO Factor out a datastructure that keeps a sorted std::vector and allows these _findLowerBound()/_findUpperBound operations using this hinted linear search
        if self.entries.is_empty() {
            return self.entries.len();
        }
        let startpos_percent =
            u32::from_le_bytes(hint.data()[..4].try_into().unwrap()) as f64 / u32::MAX as f64;
        let mut index = (startpos_percent * (self.entries.len() - 1) as f64) as usize;
        assert!(index < self.entries.len(), "Startpos out of range");

        while index != 0 && pred(&self.entries[index]) {
            index -= 1;
        }
        while index != self.entries.len() && !pred(&self.entries[index]) {
            index += 1;
        }
        index
    }

    pub fn get_by_id(&self, id: &BlobId) -> Option<&DirEntry> {
        self._get_index_by_id(id).map(|index| &self.entries[index])
    }

    pub fn deserialize<B: BlobStore>(blob: &mut BaseBlob<B>) -> Result<Self> {
        // TODO Stream this into a BufReader instead of reading everything up front? But is that actually better?
        let data = blob.read_all_data()?;
        let len = data.len() as u64;

        let mut entries = Vec::new();
        let mut reader = Cursor::new(data);
        // TODO Use reader.is_empty() once that is stabilized
        while reader.position() < len {
            let entry = DirEntry::deserialize(&mut reader)?;
            entries.push(entry);
        }
        assert_eq!(reader.position(), len, "Did not read all data.");
        Ok(Self { entries })
    }

    // TODO FusedIterator, DoubleEndedIterator
    pub fn iter(&self) -> impl Iterator<Item = &DirEntry> + ExactSizeIterator {
        self.entries.iter()
    }

    pub fn num_entries(&self) -> usize {
        self.entries.len()
    }
}
