use rand::Rng;

use crate::fuzzing::Mutator;

use super::{
    binary_level,
    tree_level::{self},
};

pub struct MutationChooser {
    binary: Vec<Box<dyn binary_level::MutateBytes>>,
    tree: Vec<Box<dyn tree_level::MutateTree>>,
}

impl Mutator for MutationChooser {
    type Item = crate::sample::Sample;

    type MutInfo = (bool, usize);

    fn mutate_sample(
        &mut self,
        mut sample: Self::Item,
        library: &[Self::Item],
    ) -> (Self::Item, Self::MutInfo) {
        let mut rng = rand::thread_rng();
        loop {
            let m1 = rng.gen_bool(0.7);
            if m1 && !self.tree.is_empty() {
                let idx = rng.gen_range(0..self.tree.len());

                let mutator = &self.tree[idx];

                match mutator.mutate(sample, library) {
                    Ok(res) => {
                        break (res, (m1, idx));
                    }
                    Err(res) => {
                        sample = res;
                    }
                }
            } else {
                let idx = rng.gen_range(0..self.binary.len());

                let mutator = &self.binary[idx];

                let new_patch = mutator.mutate(sample.get_folded(), library);

                let patched = sample.apply_patch(new_patch);

                break (patched, (m1, idx));
            }
        }
    }

    fn update_scores(&mut self, _index: Self::MutInfo, _result: crate::fuzzing::RunResult) {
        //nothing
    }
}

impl MutationChooser {
    pub fn new(
        binary: Vec<Box<dyn binary_level::MutateBytes>>,
        tree: Vec<Box<dyn tree_level::MutateTree>>,
    ) -> Self {
        MutationChooser { binary, tree }
    }
}
