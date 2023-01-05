use std::collections::HashMap;

use crate::execution::ExecutionError;

pub type OldEntries<S, E> = Vec<SampleData<S, E>>;
pub type NewEntries<S> = Vec<S>;

pub trait Generator<EvalResult>
where
    Self::Item: Sized + Clone,
{
    type Item;

    fn generate_samples(
        &mut self,
        existing_population: OldEntries<Self::Item, EvalResult>,
    ) -> (OldEntries<Self::Item, EvalResult>, NewEntries<Self::Item>);
}

pub trait Evaluator
where
    Self::Item: Sized + Clone,
{
    type Item;
    type EvalResult;
    type Error;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<SampleData<Self::Item, Self::EvalResult>, Self::Error>;
}

pub struct SampleData<Sample, EvalResult> {
    pub sample: Sample,
    pub result: EvalResult,
    pub score: f64,
}

pub type StdinSample = Vec<u8>;

pub struct Fuzzer<EvalResult> {
    sample_generator: Box<dyn Generator<EvalResult, Item = StdinSample>>,
    pub library: Vec<SampleData<StdinSample, EvalResult>>,
    evaluator:
        Box<dyn Evaluator<Item = StdinSample, EvalResult = EvalResult, Error = ExecutionError>>,
}

#[derive(Clone, Debug)]
pub struct GenerationRunResult<EvalResult> {
    pub statuses: HashMap<EvalResult, usize>,
    pub new_codes: HashMap<EvalResult, StdinSample>,
}

impl<EvalResult> Fuzzer<EvalResult>
where
    EvalResult: std::hash::Hash + Clone + Eq,
{
    pub fn new<G, E>(generator: G, evaluator: E) -> Self
    where
        G: Generator<EvalResult, Item = StdinSample> + 'static,
        E: Evaluator<Item = StdinSample, EvalResult = EvalResult, Error = ExecutionError> + 'static,
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

    pub fn run_generation(&mut self) -> Result<GenerationRunResult<EvalResult>, ExecutionError> {
        let mut library = vec![];

        std::mem::swap(&mut library, &mut self.library);

        let (mut keep, new_samples) = self.sample_generator.generate_samples(library);

        let mut scored = new_samples
            .into_iter()
            .map(|sample| self.evaluator.score(sample))
            .collect::<Result<Vec<_>, ExecutionError>>()?;

        let mut stats: HashMap<EvalResult, usize> = HashMap::new();

        let mut new_codes: HashMap<EvalResult, StdinSample> = Default::default();

        for item in &scored {
            *stats.entry(item.result.clone()).or_default() += 1;

            if item.score > 0f64 {
                new_codes.insert(item.result.clone(), item.sample.clone());
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
