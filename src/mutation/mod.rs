pub mod binary_level;
mod choice;
pub mod tree_level;

pub use tree_level::MutateTree;

pub use choice::MutationChooser;

use crate::{configuration::FuzzConfig, grammar::Grammar};

use self::{
    binary_level::{BitFlip, Erasure, Garbage, KnownBytes, MutateBytes},
    tree_level::{Resample, TreeRegrow},
};

pub fn build_mutator(_config: &FuzzConfig, grammar: &Grammar) -> MutationChooser {
    let binary: Vec<Box<dyn MutateBytes>> = vec![
        Box::new(BitFlip {}),
        Box::new(Erasure { max_size: 50 }),
        Box::new(KnownBytes::new()),
        Box::new(Garbage { max_size: 50 }),
    ];

    let tree: Vec<Box<dyn MutateTree>> = vec![
        Box::new(TreeRegrow {
            grammar: grammar.clone(),
            depth_limit: 100,
            descend_rolls: 10,
            regenerate_rolls: 10,
            mut_proba: 3,
        }),
        Box::new(Resample::new(grammar.clone(), 100)),
    ];

    MutationChooser::new(binary, tree)
}
