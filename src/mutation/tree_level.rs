use itertools::Itertools;
use rand::Rng;

use crate::{
    grammar::{
        generation::{self, Generator},
        Grammar,
    },
    sample::{Sample, TreeNode, TreeNodeItem},
};

pub trait MutateTree {
    fn mutate(&self, sample: Sample, bank: &[Sample]) -> Result<Sample, Sample>;
}

pub struct TreeRegrow {
    pub grammar: Grammar,
    pub depth_limit: usize,
    pub descend_rolls: usize,
    pub regenerate_rolls: usize,
    pub mut_proba: u32,
}

type Depth = usize;

pub(crate) fn select_random_subtree<'n>(
    root: &'n mut TreeNode,
    filter: &dyn Fn(&TreeNode) -> bool,
) -> Option<(&'n mut TreeNode, Depth)> {
    // if bugs, check here first

    let mut buf = vec![];

    writeout_nodes(root, &mut buf, 0, filter);
    if buf.is_empty() {
        return None;
    }
    let idx = rand::thread_rng().gen_range(0..buf.len());

    let (ptr, depth) = buf[idx];

    Some((unsafe { ptr.as_mut().unwrap() }, depth))
}

pub fn select_random_production(root: &mut TreeNode) -> Option<(&mut TreeNode, Depth)> {
    select_random_subtree(root, &|tree| {
        matches!(tree.item, TreeNodeItem::ProductionApplication(..))
    })
}

pub fn writeout_terminals(root: &mut TreeNode) -> Vec<&mut TreeNode> {
    let mut buf = vec![];

    fn filter(node: &TreeNode) -> bool {
        match &node.item {
            TreeNodeItem::ProductionApplication(_) => false,
            TreeNodeItem::Data(_) => true,
        }
    }

    writeout_nodes(root, &mut buf, 0, &filter);

    buf.into_iter()
        .map(|(item, _)| unsafe { item.as_mut().unwrap() })
        .collect_vec()
}

fn writeout_nodes(
    node: &mut TreeNode,
    buf: &mut Vec<(*mut TreeNode, Depth)>,
    current_depth: usize,
    filter: &dyn Fn(&TreeNode) -> bool,
) {
    if filter(node) {
        buf.push((node as *mut TreeNode, current_depth));
    }

    if let TreeNodeItem::ProductionApplication(p) = &mut node.item {
        for subnode in &mut p.items {
            writeout_nodes(subnode, buf, current_depth + 1, filter);
        }
    }
}

impl MutateTree for TreeRegrow {
    fn mutate(&self, sample: Sample, _bank: &[Sample]) -> Result<Sample, Sample> {
        // TODO keep patches in place when mutating

        let (mut tree, folded) = sample.strip();

        'reroll: for _roll in 0..self.descend_rolls {
            let Some((node, depth)) = select_random_production(&mut tree) else {
                return Err(Sample::recombine(tree, folded));
            };

            let remaining_depth = self.depth_limit - depth;

            let generator = generation::Generator::new(self.grammar.clone(), remaining_depth);

            let TreeNode{ item: TreeNodeItem::ProductionApplication(production), ..} = node else{
                continue 'reroll;
            };

            let Ok(subtree) = generator.generate_of_type(&production.rule_name, self.regenerate_rolls) else {
                continue 'reroll;
            };

            *node = TreeNode {
                item: TreeNodeItem::ProductionApplication(subtree),
                start: 0,
                size: 0,
            };

            let folded = tree.fold_into_sample();

            return Ok(folded);
        }

        Err(Sample::recombine(tree, folded))
    }
}

pub struct Resample {
    generator: Generator,
}

impl MutateTree for Resample {
    fn mutate(&self, _sample: Sample, _bank: &[Sample]) -> Result<Sample, Sample> {
        Ok(self.generator.generate())
    }
}

impl Resample {
    pub fn new(grammar: Grammar, depth_limit: usize) -> Self {
        Self {
            generator: crate::grammar::generation::Generator::new(grammar, depth_limit),
        }
    }
}
