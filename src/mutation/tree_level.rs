use rand::Rng;

use crate::{
    grammar::{
        generation::{self, ProductionApplication, TreeNodeItem},
        Grammar, TreeNode,
    },
    sample::Sample,
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

pub fn select_random_subtree<'n>(
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
    fn mutate(&self, mut sample: Sample, _bank: &[Sample]) -> Result<Sample, Sample> {
        // TODO keep patches in place when mutating

        let tree = &mut sample.tree.tree;

        'reroll: for _roll in 0..self.descend_rolls {
            let Some((node, depth)) = select_random_production(tree) else {
                return Err(sample);
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

            let folded = sample.tree.tree.fold_into_sample();

            return Ok(Sample::new(folded, sample.patches));
        }

        Err(sample)
    }
}

impl TreeRegrow {
    fn try_descend_tree<'b>(
        &self,
        mut root: &'b mut ProductionApplication,
    ) -> Result<(&'b mut ProductionApplication, usize), ()> {
        let mut remaining_depth = self.depth_limit;
        loop {
            if rand::thread_rng().gen_ratio(1, self.mut_proba) {
                return Ok((root, remaining_depth));
            } else {
                let subtrees = count_subtrees(root);
                if subtrees == 0 {
                    return Err(());
                }
                let descend_position = rand::thread_rng().gen_range(0..subtrees);

                root = find_nth_subtree(root, descend_position);
                if remaining_depth < 2 {
                    return Err(());
                } else {
                    remaining_depth -= 1;
                }
            }
        }
    }
}

fn find_nth_subtree(root: &mut ProductionApplication, mut n: usize) -> &mut ProductionApplication {
    for subnode in root.items.iter_mut() {
        match &mut subnode.item {
            TreeNodeItem::ProductionApplication(_) if n > 0 => {
                n -= 1;
            }
            TreeNodeItem::ProductionApplication(p) => {
                return p;
            }
            _ => continue,
        }
    }
    panic!()
}

fn count_subtrees(root: &ProductionApplication) -> usize {
    root.items
        .iter()
        .filter(|item| matches!(item.item, TreeNodeItem::ProductionApplication(..)))
        .count()
}
