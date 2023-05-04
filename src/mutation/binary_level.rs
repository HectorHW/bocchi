use lazy_static::lazy_static;

use rand::{distributions::WeightedIndex, prelude::Distribution, thread_rng, Rng};

pub trait MutateBytes {
    fn mutate(&self, reference: &[u8]) -> crate::sample::Patch;
}

pub fn random(size: usize) -> Vec<u8> {
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

fn apply_action(input: &mut Vec<u8>, position: usize, action: Action) {
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

pub fn mutate(reference: &Vec<u8>) -> Vec<u8> {
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

pub fn clip(mut sample: Vec<u8>, limit: usize) -> Vec<u8> {
    sample.truncate(limit);
    sample
}

pub struct RandomMutator {
    pub sample_len_limit: usize,
}

impl RandomMutator {
    fn mutate(&self, sample: Vec<u8>) -> Vec<u8> {
        clip(mutate(&sample), self.sample_len_limit)
    }
}

fn get_random_position(buffer: &[u8]) -> usize {
    if buffer.is_empty() {
        return 0;
    }
    let mut rng = rand::thread_rng();

    rng.gen_range(0..buffer.len())
}

pub struct BitFlip {}

impl MutateBytes for BitFlip {
    fn mutate(&self, reference: &[u8]) -> crate::sample::Patch {
        let mut rng = rand::thread_rng();

        let random_bit = 1 << (rng.gen_range(0..8));

        if reference.is_empty() {
            return crate::sample::Patch::Xor {
                position: 0,
                content: vec![random_bit],
            };
        }

        let random_position = get_random_position(reference);

        crate::sample::Patch::Xor {
            position: random_position,
            content: vec![random_bit],
        }
    }
}

pub struct Erasure {
    pub max_size: usize,
}

impl MutateBytes for Erasure {
    fn mutate(&self, reference: &[u8]) -> crate::sample::Patch {
        let mut rng = rand::thread_rng();

        let random_size = rng.gen_range(1..=self.max_size);
        let random_position = get_random_position(reference);

        crate::sample::Patch::Erasure {
            position: random_position,
            size: random_size,
        }
    }
}
