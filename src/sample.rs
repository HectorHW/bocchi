use std::{io::Write, ops::Range};

use crate::{mutation::tree_level::writeout_terminals, sample_library::SizeScore};

#[derive(Clone, Debug)]
pub struct Patch {
    pub position: usize,
    pub kind: PatchKind,
}

#[derive(Clone, Debug)]
pub enum PatchKind {
    Replacement(Vec<u8>),

    Erasure(usize),

    Insertion(Vec<u8>),
}

fn intersect_intervals(first: (usize, usize), second: (usize, usize)) -> Option<Range<usize>> {
    if first.0 > second.1 || second.0 > first.1 {
        return None;
    }

    let interval = first.0.max(second.0)..first.1.min(second.1);

    if interval.is_empty() {
        None
    } else {
        Some(interval)
    }
}

fn remap_interval_to_segment(range: Range<usize>, data_start: usize) -> Range<usize> {
    range.start - data_start..range.end - data_start
}

#[derive(Clone, Debug)]
pub struct ProductionApplication {
    pub rule_name: String,
    pub production_variant: usize,
    pub items: Vec<TreeNode>,
}

#[derive(Clone, Debug)]
pub struct TreeNode {
    pub start: usize,
    pub size: usize,
    pub item: TreeNodeItem,
}

#[derive(Clone, Debug)]
pub enum TreeNodeItem {
    ProductionApplication(ProductionApplication),
    Data(Vec<u8>),
}

impl TreeNodeItem {
    fn find_tree_span(&self) -> usize {
        match self {
            TreeNodeItem::ProductionApplication(p) => p.items.iter().map(|item| item.size).sum(),
            TreeNodeItem::Data(data) => data.len(),
        }
    }

    fn with_offset(self, offset: usize) -> TreeNode {
        TreeNode {
            start: offset,
            size: self.find_tree_span(),
            item: self,
        }
    }
}

impl From<TreeNodeItem> for TreeNode {
    fn from(value: TreeNodeItem) -> Self {
        TreeNode {
            start: 0,
            size: value.find_tree_span(),
            item: value,
        }
    }
}

impl TreeNode {
    /// write this tree to buffer setting indices in the process
    pub fn fold(&mut self, buffer: &mut Vec<u8>) {
        let before = buffer.len();
        match &mut self.item {
            TreeNodeItem::ProductionApplication(pa) => {
                for item in &mut pa.items {
                    item.fold(buffer);
                }
            }
            TreeNodeItem::Data(data) => {
                buffer.write_all(data).unwrap();
            }
        }
        self.start = before;
        self.size = buffer.len() - before;
    }

    pub fn fold_into_sample(mut self) -> GrammarSample {
        let mut buf = vec![];

        self.fold(&mut buf);

        GrammarSample {
            tree: self,
            folded: buf,
        }
    }
}

impl From<TreeNode> for GrammarSample {
    fn from(mut val: TreeNode) -> Self {
        let mut folded = vec![];
        val.fold(&mut folded);
        GrammarSample { tree: val, folded }
    }
}

#[derive(Clone, Debug)]
pub struct GrammarSample {
    tree: TreeNode,
    folded: Vec<u8>,
}

pub type Sample = GrammarSample;

fn apply_patch(data: &mut Vec<u8>, data_pos: usize, patch: &Patch) {
    if data.is_empty() {
        return;
    }

    match &patch.kind {
        PatchKind::Replacement(content) => {
            let Some(span_in_data) = intersect_intervals(
                (data_pos, data_pos + data.len()),
                (patch.position, patch.position + content.len()),
            ) else {
                return;
            };

            data[remap_interval_to_segment(span_in_data.clone(), data_pos)]
                .copy_from_slice(&content[remap_interval_to_segment(span_in_data, patch.position)]);
        }
        PatchKind::Erasure(size) => {
            let Some(span_in_data) = intersect_intervals(
                (data_pos, data_pos + data.len()),
                (patch.position, patch.position + size),
            ) else {
                return;
            };

            let mut prefix =
                data[..remap_interval_to_segment(span_in_data.clone(), data_pos).start].to_owned();
            let mut suffix =
                data[remap_interval_to_segment(span_in_data, data_pos).end..].to_owned();

            let remaining_data = {
                prefix.append(&mut suffix);
                prefix
            };

            *data = remaining_data;
        }
        PatchKind::Insertion(content) => {
            if patch.position >= data_pos && patch.position < data_pos + data.len() {
                let span = remap_interval_to_segment(patch.position..patch.position + 1, data_pos);

                let mut suffix = data.split_off(span.start);

                let mut insertion = content.clone();

                data.append(&mut insertion);
                data.append(&mut suffix);
            }
        }
    }
}

impl Sample {
    pub fn get_folded(&self) -> &[u8] {
        &self.folded
    }

    pub fn strip(self) -> (TreeNode, Vec<u8>) {
        (self.tree, self.folded)
    }

    pub fn recombine(tree: TreeNode, folded: Vec<u8>) -> Self {
        Self { tree, folded }
    }

    pub fn apply_patch(mut self, patch: Patch) -> Self {
        for terminal in writeout_terminals(&mut self.tree) {
            let TreeNode{item: TreeNodeItem::Data(data), start,..} = terminal else {
                unreachable!()
            };

            apply_patch(data, *start, &patch)
        }

        self.tree.fold_into_sample()
    }
}

impl SizeScore for Sample {
    fn get_size_score(&self) -> usize {
        self.folded.len()
    }
}
