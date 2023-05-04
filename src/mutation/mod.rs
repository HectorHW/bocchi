pub mod binary_level;
mod choice;
pub mod tree_level;

pub use tree_level::MutateTree;

pub use choice::MutationChooser;

use crate::{configuration::FuzzConfig, fuzzing::Mutator, grammar::Grammar};

use self::{
    binary_level::{BitFlip, Erasure, MutateBytes},
    tree_level::TreeRegrow,
};

pub fn build_mutator(
    config: &FuzzConfig,
    grammar: &Grammar,
) -> Box<dyn Mutator<Item = crate::sample::Sample, MutInfo = (bool, usize)>> {
    let binary: Vec<Box<dyn MutateBytes>> =
        vec![Box::new(BitFlip {}), Box::new(Erasure { max_size: 50 })];

    let tree: Vec<Box<dyn MutateTree>> = vec![Box::new(TreeRegrow {
        grammar: grammar.clone(),
        depth_limit: 100,
        descend_rolls: 10,
        regenerate_rolls: 10,
        mut_proba: 3,
    })];

    Box::new(MutationChooser::new(binary, tree))
}
