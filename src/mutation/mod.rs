pub mod binary_level;
mod choice;
pub mod tree_level;

pub use tree_level::MutateTree;

pub use choice::MutationChooser;

use crate::{
    configuration::{FuzzConfig, InputOptions},
    grammar::Grammar,
};

use self::{
    binary_level::{BitFlip, CopyFragment, Erasure, Garbage, KnownBytes, MutateBytes},
    tree_level::{Resample, TreeRegrow},
};

pub fn build_mutator(config: &FuzzConfig, grammar: &Grammar) -> MutationChooser {
    let binary: Vec<Box<dyn MutateBytes>> = vec![
        Box::new(BitFlip {}),
        Box::new(Erasure { max_size: 100 }),
        Box::new(KnownBytes::new()),
        Box::new(Garbage { max_size: 20 }),
        Box::new(CopyFragment { max_size: 100 }),
    ];

    let tree: Vec<Box<dyn MutateTree>> = if matches!(config.input, InputOptions::Grammar { .. }) {
        vec![
            Box::new(TreeRegrow {
                grammar: grammar.clone(),
                depth_limit: 100,
                descend_rolls: 10,
                regenerate_rolls: 10,
                mut_proba: 3,
            }),
            Box::new(Resample::new(grammar.clone(), 100)),
        ]
    } else {
        vec![]
    };

    MutationChooser::new(binary, tree)
}
