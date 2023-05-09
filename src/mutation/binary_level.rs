use lazy_static::lazy_static;

use rand::{distributions::WeightedIndex, Rng};

use crate::sample::{Patch, PatchKind};

pub trait MutateBytes {
    fn mutate(&self, reference: &[u8]) -> Patch;
}

lazy_static! {
    static ref DECREASING_WEIGHTS_DIST: WeightedIndex<usize> =
        WeightedIndex::new((1..=20).rev().map(|amount| amount * 3 / 2)).unwrap();
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
    fn mutate(&self, reference: &[u8]) -> Patch {
        let mut rng = rand::thread_rng();

        let random_bit = 1 << (rng.gen_range(0..8));

        let position = if reference.is_empty() {
            0
        } else {
            get_random_position(reference)
        };

        let new_data = if reference.is_empty() {
            random_bit
        } else {
            random_bit ^ reference[position]
        };

        Patch {
            position,
            kind: PatchKind::Replacement(vec![new_data]),
        }
    }
}

pub struct Erasure {
    pub max_size: usize,
}

impl MutateBytes for Erasure {
    fn mutate(&self, reference: &[u8]) -> Patch {
        let mut rng = rand::thread_rng();

        let random_size = rng.gen_range(1..=self.max_size);
        let random_position = get_random_position(reference);

        Patch {
            position: random_position,
            kind: PatchKind::Erasure(random_size),
        }
    }
}

pub struct KnownBytes {
    variants: Vec<Vec<u8>>,
}

impl MutateBytes for KnownBytes {
    fn mutate(&self, reference: &[u8]) -> Patch {
        if reference.is_empty() {
            return Patch {
                position: 0,
                kind: PatchKind::Replacement(vec![0x00]),
            };
        }
        let mut rng = rand::thread_rng();
        let item = rng.gen_range(0..self.variants.len());
        let position = rng.gen_range(0..reference.len());

        let mut content = self.variants[item].clone();

        let swap_endianness = rng.gen_bool(0.5);

        if swap_endianness {
            content.reverse()
        }

        Patch {
            position,
            kind: PatchKind::Replacement(content),
        }
    }
}

impl KnownBytes {
    pub fn new() -> Self {
        Self {
            variants: vec![
                vec![0x00],
                vec![0xff],
                vec![0x00, 0x00],
                vec![0xff, 0xff],
                vec![0x00, 0x00, 0x00, 0x00],
                vec![0xff, 0xff, 0xff, 0xff],
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                vec![0x7f],
                vec![0x01],
                vec![0xf0],
                vec![0x00, 0x00, 0x00, 0x80],
                vec![0x00, 0x00, 0x00, 0x40],
            ],
        }
    }
}

pub struct Garbage {
    pub max_size: usize,
}

impl MutateBytes for Garbage {
    fn mutate(&self, reference: &[u8]) -> Patch {
        let mut rng = rand::thread_rng();

        let size = rng.gen_range(1..=self.max_size);

        let content = (0..size).map(|_| rng.gen()).collect();

        if reference.is_empty() {
            Patch {
                position: 0,
                kind: PatchKind::Replacement(content),
            }
        } else {
            let position = rng.gen_range(0..reference.len());
            Patch {
                position,
                kind: PatchKind::Replacement(content),
            }
        }
    }
}
