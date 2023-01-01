use std::collections::HashMap;

use crate::execution::ExecutionError;

pub trait Generator
where
    Self::Item: Sized + Clone,
{
    type Item;
    fn generate_samples(
        &mut self,
        existing_population: Vec<SampleData<Self::Item>>,
    ) -> (Vec<SampleData<Self::Item>>, Vec<Self::Item>);
}

pub trait Evaluator
where
    Self::Item: Sized + Clone,
{
    type Item;
    type Error;

    fn score(&mut self, sample: Self::Item) -> Result<SampleData<Self::Item>, Self::Error>;
}

pub struct SampleData<Sample> {
    pub sample: Sample,
    pub score: f64,
    pub return_code: i32,
}

pub type StdinSample = Vec<u8>;

pub struct Fuzzer {
    sample_generator: Box<dyn Generator<Item = StdinSample>>,
    pub library: Vec<SampleData<StdinSample>>,
    evaluator: Box<dyn Evaluator<Item = StdinSample, Error = ExecutionError>>,
}

#[derive(Clone, Debug)]
pub struct GenerationRunResult {
    pub statuses: HashMap<i32, usize>,
    pub new_codes: HashMap<i32, StdinSample>,
}

impl Fuzzer {
    pub fn new<G, E>(generator: G, evaluator: E) -> Self
    where
        G: Generator<Item = StdinSample> + 'static,
        E: Evaluator<Item = StdinSample, Error = ExecutionError> + 'static,
    {
        Fuzzer {
            sample_generator: Box::new(generator),
            library: vec![],
            evaluator: Box::new(evaluator),
        }
    }

    pub fn add_to_library(&mut self, sample: StdinSample) -> Result<(), ExecutionError> {
        let scored = self.evaluator.score(sample)?;
        self.library.push(scored);
        Ok(())
    }

    pub fn run_generation(&mut self) -> Result<GenerationRunResult, ExecutionError> {
        let mut library = vec![];

        std::mem::swap(&mut library, &mut self.library);

        let (mut keep, new_samples) = self.sample_generator.generate_samples(library);

        let mut scored = new_samples
            .into_iter()
            .map(|sample| self.evaluator.score(sample))
            .collect::<Result<Vec<_>, ExecutionError>>()?;

        let mut stats: HashMap<i32, usize> = HashMap::new();

        let mut new_codes: HashMap<i32, StdinSample> = Default::default();

        for item in &scored {
            *stats.entry(item.return_code).or_default() += 1;

            if item.score > 0f64 {
                new_codes.insert(item.return_code, item.sample.clone());
            }
        }

        keep.append(&mut scored);

        self.library = keep;

        Ok(GenerationRunResult {
            statuses: stats,
            new_codes,
        })
    }
}
