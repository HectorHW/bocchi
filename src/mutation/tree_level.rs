use rand::Rng;

use crate::grammar::{
    generation::{self, ProductionApplication},
    Grammar, Sample, TreeNode,
};

pub trait MutateTree {
    fn mutate(&mut self, sample: Sample, bank: &[Sample]) -> Result<Sample, Sample>;
}

pub struct TreeRegrow {
    pub grammar: Grammar,
    pub depth_limit: usize,
    pub descend_rolls: usize,
    pub regenerate_rolls: usize,
    pub mut_proba: u32,
}

impl MutateTree for TreeRegrow {
    fn mutate(&mut self, mut sample: Sample, _bank: &[Sample]) -> Result<Sample, Sample> {
        let tree = &mut sample.tree;

        'reroll: for _roll in 0..self.descend_rolls {
            let TreeNode::ProductionApplication(root) = tree else {
                return Err(sample);
            };

            let Ok((root, remaining_depth)) = self.try_descend_tree(root) else {
                continue 'reroll;
            };

            let generator = generation::Generator::new(self.grammar.clone(), remaining_depth);

            let Ok(subtree) = generator.generate_of_type(&root.rule_name, self.regenerate_rolls) else {
                continue 'reroll;
            };

            *root = subtree;
            let mut folded = vec![];
            sample.tree.fold(&mut folded);
            sample.folded = folded;
            return Ok(sample);
        }

        Err(sample)
    }
}

impl TreeRegrow {
    fn try_descend_tree<'b>(
        &mut self,
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
        match subnode {
            TreeNode::ProductionApplication(_) if n > 0 => {
                n -= 1;
            }
            TreeNode::ProductionApplication(p) => {
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
        .filter(|item| matches!(item, TreeNode::ProductionApplication(..)))
        .count()
}
