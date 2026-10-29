use anyhow::{Result, ensure};
use binary_layout::Field;
use log::trace;
use std::fmt::Debug;
use std::num::{NonZeroU8, NonZeroU32};

use super::DataNode;
use super::layout::{FORMAT_VERSION_HEADER, NodeLayout, node};
use crate::blockstore::{BLOCKID_LEN, Block, BlockId, BlockStore};
use cryfs_utils::data::{Data, ZeroedData};

pub(super) const MAX_DEPTH: u8 = 10;

pub struct DataInnerNode<B: BlockStore> {
    block: B::Block,
}

impl<B: BlockStore> DataInnerNode<B> {
    pub fn new(block: B::Block, layout: &NodeLayout) -> Result<Self> {
        trace!("Block node type: Inner (depth > 0)");
        // Min block size: enough for header and for inner nodes to have at least two children and form a tree.
        let min_block_size = node::data::OFFSET + 2 * BLOCKID_LEN;
        assert!(
            usize::try_from(layout.block_size.as_u64()).unwrap() >= min_block_size,
            "Block doesn't have enough space for header and two children. This should have been checked before calling DataInnerNode::new"
        );

        let view = node::View::new(block.data());
        ensure!(
            view.format_version_header().read() == FORMAT_VERSION_HEADER,
            "Loaded a node with format version {} but the current version is {}",
            view.format_version_header().read(),
            FORMAT_VERSION_HEADER,
        );
        let depth = view.depth().read();
        assert_ne!(
            0, depth,
            "Loaded an inner node with depth 0. This doesn't make sense, it should have been loaded as a leaf node",
        );
        ensure!(
            block.data().len() == usize::try_from(layout.block_size.as_u64()).unwrap(),
            "Loaded block of size {} but expected {}",
            block.data().len(),
            layout.block_size,
        );
        ensure!(
            depth <= MAX_DEPTH,
            "Loaded an inner node with depth {} but the maximum is {}",
            depth,
            MAX_DEPTH,
        );
        let size = view.size().read();
        let max_children_per_inner_node = layout.max_children_per_inner_node();
        ensure!(
            size >= 1,
            "Loaded an inner node that claims to store {} children but the minimum is 1.",
            size,
        );
        ensure!(
            size <= max_children_per_inner_node,
            "Loaded an inner node that claims to store {} children but the maximum is {}.",
            size,
            max_children_per_inner_node,
        );
        Ok(Self { block })
    }

    pub fn depth(&self) -> NonZeroU8 {
        let view = node::View::new(self.block.data());
        NonZeroU8::new(view.depth().read())
            .expect("DataInnerNode class invariant violated: Has depth of zero")
    }

    pub fn block_id(&self) -> &BlockId {
        self.block.block_id()
    }

    pub(super) fn raw_blockdata(&self) -> &Data {
        self.block.data()
    }

    pub(super) fn into_block(self) -> B::Block {
        self.block
    }

    pub fn num_children(&self) -> NonZeroU32 {
        let view = node::View::new(self.block.data().as_ref());
        NonZeroU32::new(view.size().read())
            .expect("DataInnerNode class invariant violated: Has only zero children")
    }

    pub fn children(&self) -> impl ExactSizeIterator<Item = BlockId> {
        let view = node::View::new(self.block.data().as_ref());
        let num_children = usize::try_from(view.size().read()).unwrap();
        let children_data = view.into_data().into_slice();
        let children_ids = children_data.chunks_exact(BLOCKID_LEN);
        assert!(
            num_children <= children_ids.len(),
            "Class invariant violated: Tried to load an inner node with {} children but support at most {} per inner node",
            num_children,
            children_ids.len(),
        );
        children_ids
            .take(num_children)
            .map(|id_bytes| BlockId::from_slice(id_bytes).unwrap())
    }

    pub fn upcast(self) -> DataNode<B> {
        DataNode::Inner(self)
    }
}

pub fn serialize_inner_node(depth: u8, children: &[BlockId], layout: &NodeLayout) -> Data {
    let data = ZeroedData::new(usize::try_from(layout.block_size.as_u64()).unwrap());
    initialize_inner_node(depth, children, layout, data)
}

pub fn initialize_inner_node<D>(
    depth: u8,
    children: &[BlockId],
    layout: &NodeLayout,
    dest: ZeroedData<D>,
) -> D
where
    D: AsRef<[u8]> + AsMut<[u8]>,
{
    assert!(
        depth != 0,
        "Inner node cannot have a depth of 0. Is this perhaps a leaf instead?"
    );
    assert!(
        depth <= MAX_DEPTH,
        "Inner node cannot have a depth of {}, the maximum is {}",
        depth,
        MAX_DEPTH,
    );
    assert!(
        !children.is_empty(),
        "Inner node must have at least one child"
    );
    assert!(
        children.len() <= usize::try_from(layout.max_children_per_inner_node()).unwrap(),
        "Inner nodes can only store {} children but tried to store {}",
        layout.max_children_per_inner_node(),
        children.len(),
    );

    let mut view = node::View::new(dest.into_inner());
    view.format_version_header_mut()
        .write(FORMAT_VERSION_HEADER);
    view.unused_mut().write(0);
    view.depth_mut().write(depth);
    view.size_mut()
        .write(u32::try_from(children.len()).unwrap());
    _serialize_children(view.data_mut(), children);
    view.into_storage()
}

fn _serialize_children(dest: &mut [u8], children: &[BlockId]) {
    assert!(
        dest.len() >= children.len() * BLOCKID_LEN,
        "Serializing {} children requires {} bytes but tried to serialize into a buffer with {} bytes.",
        children.len(),
        children.len() * BLOCKID_LEN,
        dest.len()
    );
    for (index, child) in children.iter().enumerate() {
        // TODO Some way to avoid this copy by not using &[BlockId] or Vec<BlockId> but our own collection type that already has it aligned correctly?
        dest[(BLOCKID_LEN * index)..(BLOCKID_LEN * (index + 1))].copy_from_slice(child.data());
    }
}

impl<B: BlockStore> Debug for DataInnerNode<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataInnerNode")
            .field("block_id", &self.block_id())
            .field("depth", &self.depth())
            .field("children", &self.children().collect::<Vec<_>>())
            .finish()
    }
}
