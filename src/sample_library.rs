use itertools::Itertools;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use vector_map::VecMap;

pub trait Library {
    type Key: Clone + Eq + CoverageScore;
    type Item: Sized + Clone;

    fn find_existing(&self, reference: &Self::Key) -> Option<&LibraryEntry<Self::Item>>;

    fn upsert(&mut self, key: Self::Key, object: Self::Item);

    fn add_name(&mut self, key: &Self::Key, name: String);

    fn pick_random(&self) -> Self::Item;

    fn linearize(&mut self) -> &[Self::Item];
}

pub struct LibraryEntry<V> {
    pub item: V,
    index: usize,
    pub unique_name: Option<String>,
}

pub struct VectorLibrary<K, V> {
    /// cached contiguous items array
    items: Vec<V>,
    buffer: vector_map::VecMap<K, LibraryEntry<V>>,
}

pub trait CoverageScore {
    fn get_score(&self) -> f64;
}

pub trait SizeScore {
    fn get_size_score(&self) -> f64;
}

impl<K: Clone + CoverageScore + Eq, V: Clone + SizeScore> Library for VectorLibrary<K, V> {
    type Item = V;
    type Key = K;

    fn find_existing(&self, reference: &Self::Key) -> Option<&LibraryEntry<Self::Item>> {
        self.buffer.get(reference)
    }

    fn upsert(&mut self, key: Self::Key, object: Self::Item) {
        if let Some(exisiting) = self.buffer.get_mut(&key) {
            exisiting.item = object.clone();
            self.items[exisiting.index] = object;
        } else {
            let index = self.items.len();

            self.buffer.insert(
                key,
                LibraryEntry {
                    item: object.clone(),
                    index,
                    unique_name: None,
                },
            );
            self.items.push(object)
        }
    }

    fn add_name(&mut self, key: &Self::Key, name: String) {
        let Some(existing) = self.buffer.get_mut(key) else{
            panic!("called add_name without prior upsert");
        };

        existing.unique_name = Some(name);
    }

    fn pick_random(&self) -> Self::Item {
        let weights = self
            .buffer
            .keys()
            .map(CoverageScore::get_score)
            .collect_vec();

        let dist = WeightedIndex::new(&weights).unwrap();

        let mut rng = thread_rng();

        self.items[dist.sample(&mut rng)].clone()
    }

    fn linearize(&mut self) -> &[Self::Item] {
        &self.items
    }
}

impl<K: Eq, V> VectorLibrary<K, V> {
    pub fn new() -> Self {
        Self {
            buffer: VecMap::new(),
            items: vec![],
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &LibraryEntry<V>)> {
        self.buffer.iter()
    }
}
