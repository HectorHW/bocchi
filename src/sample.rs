use std::io::Write;

use crate::sample_library::SizeScore;

#[derive(Clone, Debug)]
pub enum Patch {
    Replacement { position: usize, content: Vec<u8> },

    Xor { position: usize, content: Vec<u8> },

    Erasure { position: usize, size: usize },

    Insertion { position: usize, content: Vec<u8> },
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

            Patch::Insertion { position, content } => {
                todo!()
            }
        }
    }
}

enum PatchPartition {
    Unaffected,
    Left(usize),
    Right(usize),
    Split { head: usize, tail: usize },
    Overshadowed,
}

fn partition_patch_over_hole(
    start: usize,
    size: usize,
    hole_start: usize,
    hole_size: usize,
) -> PatchPartition {
    if start + size < hole_start || hole_start + hole_size < start {
        return PatchPartition::Unaffected;
    }

    if start < hole_start && start + size < hole_start + hole_size {
        return PatchPartition::Left(hole_start - start);
    }

    if start >= hole_start && start + size > hole_start + hole_size {
        return PatchPartition::Right(start + size - hole_start - hole_size);
    }

    if start >= hole_start && start + size <= hole_start + hole_size {
        return PatchPartition::Overshadowed;
    }

    PatchPartition::Split {
        head: hole_start - start,
        tail: start + size - hole_start - hole_size,
    }
}

pub fn remap_patches(
    patches: Vec<Patch>,
    mut sample_size: usize,
    mut hole_start: usize,
    hole_size: usize,
) -> Vec<Patch> {
    let mut result = vec![];

    for patch in patches {
        match patch {
            Patch::Replacement { .. } | Patch::Xor { .. } => {}

            Patch::Erasure {
                mut position,
                mut size,
            } => {
                match partition_patch_over_hole(position, size, hole_start, hole_size) {
                    PatchPartition::Unaffected => {
                        result.push(patch);
                    }
                    PatchPartition::Left(left) => result.push(Patch::Erasure {
                        position,
                        size: left,
                    }),
                    PatchPartition::Right(right) => result.push(Patch::Erasure {
                        position: position + size - right,
                        size: right,
                    }),
                    PatchPartition::Split { head, tail } => todo!(),
                    PatchPartition::Overshadowed => {
                        continue;
                    }
                }

                if position < hole_start {
                    hole_start -= size;
                }
                if position + size < sample_size {
                    sample_size -= size;
                } else {
                    sample_size = position;
                }
            }

            Patch::Insertion { position, content } => todo!(),
        }
    }

    result
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub tree: crate::grammar::GrammarSample,
    pub patches: Vec<Patch>,
    pub result: Vec<u8>,
}

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
