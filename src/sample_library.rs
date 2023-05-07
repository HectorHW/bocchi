use std::fmt::Debug;

use itertools::Itertools;
use rand::distributions::WeightedIndex;
use rand::prelude::*;

use crate::execution::DetailedTrace;

pub trait Library {
    type Key: ComparisonKey + Clone;
    type Item: Sized + Clone;

    fn find_existing(&self, reference: &Self::Key) -> Option<&Self::Item>;

    fn find_existing_mut(&mut self, reference: &Self::Key) -> Option<&mut Self::Item>;

    fn upsert(&mut self, key: Self::Key, object: Self::Item);

    fn attach_detailed_trace(&mut self, key: &Self::Key, trace: DetailedTrace);

    fn get_detailed_trace(&self, key: &Self::Key) -> Option<&DetailedTrace>;

    fn pick_random(&self) -> Self::Item;

    fn linearize(&mut self) -> &[Self::Item];

    fn write(&self) -> String;
}

pub struct VectorLibrary<K, V> {
    keys: Vec<K>,
    items: Vec<V>,
    detailed_traces: Vec<Option<DetailedTrace>>,
}

pub trait ComparisonKey {
    type Key: Eq;

    fn get_key(&self) -> &Self::Key;
}

pub trait CoverageScore {
    fn get_score(&self) -> f64;
}

pub trait SizeScore {
    fn get_size_score(&self) -> f64;
}

impl<K: Clone + ComparisonKey + CoverageScore + Debug, V: Clone + SizeScore + Debug> Library
    for VectorLibrary<K, V>
{
    type Item = V;
    type Key = K;

    fn find_existing(&self, reference: &Self::Key) -> Option<&Self::Item> {
        let reference = reference.get_key();
        self.keys
            .iter()
            .zip(self.items.iter())
            .find(|(k, _)| k.get_key() == reference)
            .map(|(_, v)| v)
    }

    fn find_existing_mut(&mut self, reference: &Self::Key) -> Option<&mut Self::Item> {
        let reference = reference.get_key();
        self.keys
            .iter()
            .zip(self.items.iter_mut())
            .find(|(k, _)| k.get_key() == reference)
            .map(|(_, v)| v)
    }

    fn upsert(&mut self, key: Self::Key, object: Self::Item) {
        if let Some(existing) = self.find_existing_mut(&key) {
            *existing = object;
        } else {
            self.keys.push(key);
            self.items.push(object);
            self.detailed_traces.push(None)
        }
    }

    fn pick_random(&self) -> Self::Item {
        let weights = self.keys.iter().map(CoverageScore::get_score).collect_vec();

        let dist = WeightedIndex::new(&weights).unwrap();

        let mut rng = thread_rng();

        self.items[dist.sample(&mut rng)].clone()
    }

    fn linearize(&mut self) -> &[Self::Item] {
        &self.items
    }

    fn write(&self) -> String {
        self.items
            .iter()
            .zip(self.keys.iter())
            .map(|(v, k)| format!("{k:?} => {v:?}"))
            .join("\n")
    }

    fn attach_detailed_trace(&mut self, key: &Self::Key, trace: DetailedTrace) {
        let (item_idx, _) = self
            .keys
            .iter()
            .find_position(|k| k.get_key() == key.get_key())
            .unwrap();

        self.detailed_traces[item_idx] = Some(trace)
    }

    fn get_detailed_trace(&self, key: &Self::Key) -> Option<&DetailedTrace> {
        let (item_idx, _) = self
            .keys
            .iter()
            .find_position(|k| k.get_key() == key.get_key())
            .unwrap();

        self.detailed_traces[item_idx].as_ref()
    }
}

impl<K, V> VectorLibrary<K, V> {
    pub fn new() -> Self {
        Self {
            keys: vec![],
            items: vec![],
            detailed_traces: vec![],
        }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    pub fn values(&self) -> &[V] {
        &self.items
    }
}
