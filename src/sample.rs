use std::io::Write;

use crate::sample_library::SizeScore;

#[derive(Clone, Debug)]
pub enum Patch {
    Replacement { position: usize, content: Vec<u8> },

    Xor { position: usize, content: Vec<u8> },

    Erasure { position: usize, size: usize },
}

impl Patch {
    fn apply(&self, target: &mut Vec<u8>) {
        match self {
            Patch::Replacement { position, content } => {
                let start = *position;
                let end = (position + content.len()).min(target.len());

                let allowed_size = end - start;

                target[start..end].copy_from_slice(&content[0..allowed_size]);
            }
            Patch::Xor { position, content } => {
                let start = *position;
                let end = (position + content.len()).min(target.len());

                let allowed_size = end - start;

                for byte_pos in 0..allowed_size {
                    target[start + byte_pos] ^= content[byte_pos];
                }
            }
            &Patch::Erasure { position, size } => {
                let mut new = target[0..position.min(target.len())].to_vec();

                if position + size < target.len() {
                    new.write_all(&target[(position + size)..target.len()])
                        .unwrap();
                }
                *target = new;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sample {
    tree: crate::grammar::GrammarSample,
    patches: Vec<Patch>,
    result: Vec<u8>,
}

pub type TestedSample = crate::fuzzing::TestedSample<Sample, crate::execution::RunTrace>;

impl Sample {
    pub fn new(tree: crate::grammar::GrammarSample, patches: Vec<Patch>) -> Self {
        let mut result = tree.folded.clone();

        for patch in &patches {
            patch.apply(&mut result);
        }

        Sample {
            tree,
            patches,
            result,
        }
    }

    pub fn get_folded(&self) -> &[u8] {
        &self.result
    }

    pub fn append_patch(self, patch: Patch) -> Self {
        Self::new(self.tree, {
            let mut patches = self.patches;
            patches.push(patch);
            patches
        })
    }

    pub fn strip(self) -> (crate::grammar::GrammarSample, Vec<Patch>) {
        (self.tree, self.patches)
    }
}

impl SizeScore for Sample {
    fn get_size_score(&self) -> f64 {
        self.result.len() as f64
    }
}
