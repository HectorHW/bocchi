use lazy_static::lazy_static;
use std::iter::repeat_with;

use rand::{distributions::WeightedIndex, prelude::Distribution, thread_rng, Rng};

use crate::fuzzing::Generator;

pub type InputSample = Vec<u8>;

pub fn random(size: usize) -> InputSample {
    (0..size).map(|_| thread_rng().gen()).collect()
}

#[derive(Clone, Copy)]
enum Action {
    Deletion(usize),
    Insertion(usize),
    Replacement(usize),
}

lazy_static! {
    static ref DECREASING_WEIGHTS_DIST: WeightedIndex<usize> =
        WeightedIndex::new((1..=20).rev().map(|amount| amount * 3 / 2)).unwrap();
}

fn get_random_action(remaining_input: usize) -> Action {
    let mut rng = thread_rng();

    let size = DECREASING_WEIGHTS_DIST.sample(&mut thread_rng());
    let clipped_size = size.min(remaining_input);

    match rng.gen_range(0..3) {
        0 => Action::Deletion(clipped_size),
        1 => Action::Insertion(size),
        2 => Action::Replacement(clipped_size),
        _ => unreachable!(),
    }
}

fn action_with_chance(inv_chance: u32, remaining_input: usize) -> Option<Action> {
    if thread_rng().gen_ratio(1, inv_chance) {
        Some(get_random_action(remaining_input))
    } else {
        None
    }
}

fn apply_action(input: &mut InputSample, position: usize, action: Action) {
    let mut new_buf = input[..position].to_owned();

    match action {
        Action::Deletion(deletion_size) => {
            new_buf.append(&mut input[(position + deletion_size)..].to_owned());
        }
        Action::Insertion(insertion_size) => {
            for _ in 0..insertion_size {
                new_buf.push(thread_rng().gen());
            }
            new_buf.append(&mut input[(position)..].to_owned());
        }
        Action::Replacement(size) => {
            for _ in 0..size {
                new_buf.push(thread_rng().gen());
            }
            new_buf.append(&mut input[(position + size)..].to_owned());
        }
    }
    *input = new_buf;
}

pub fn mutate(reference: &InputSample) -> InputSample {
    let mut data = reference.clone();

    let mut idx = 0;

    let action_chance = (reference.len() as u32 / 2).max(2);

    while idx < data.len() {
        if let Some(action) = action_with_chance(action_chance, data.len() - idx) {
            apply_action(&mut data, idx, action);

            match action {
                Action::Deletion(_) => {}
                Action::Insertion(size) => idx += size,
                Action::Replacement(size) => idx += size,
            }
        } else {
            idx += 1;
        }
    }

    data
}

pub fn clip(mut sample: InputSample, limit: usize) -> InputSample {
    sample.truncate(limit);
    sample
}

pub struct RandomMutator {
    pub generation_size: usize,
    pub sample_len_limit: usize,
}

impl<EvalResult> Generator<EvalResult> for RandomMutator {
    type Item = Vec<u8>;

    fn generate_samples(
        &mut self,
        mut existing_population: Vec<crate::fuzzing::SampleData<Self::Item, EvalResult>>,
    ) -> (
        Vec<crate::fuzzing::SampleData<Self::Item, EvalResult>>,
        Vec<Self::Item>,
    ) {
        assert!(!existing_population.is_empty());

        existing_population.truncate(self.generation_size);

        let to_keep = if existing_population.len() > 2 {
            existing_population
                .into_iter()
                .filter(|_item| thread_rng().gen_ratio(1, 2))
                .collect::<Vec<_>>()
        } else {
            existing_population
        };

        let new = repeat_with(|| {
            let random_item = &to_keep[thread_rng().gen_range(0..to_keep.len())];

            clip(mutate(&random_item.sample), self.sample_len_limit)
        })
        .take(self.generation_size - to_keep.len())
        .collect();

        (to_keep, new)
    }
}
