use std::collections::{HashMap, HashSet};

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
    Self::Error: std::error::Error,
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

pub trait Sample: std::hash::Hash + Clone + Eq {}

impl Sample for StdinSample {}

pub struct Fuzzer<EvalResult, Err> {
    sample_generator: Box<dyn Generator<EvalResult, Item = StdinSample>>,
    pub library: Vec<SampleData<StdinSample, EvalResult>>,
    evaluator: Box<dyn Evaluator<Item = StdinSample, EvalResult = EvalResult, Error = Err>>,
    unique_crashes: HashSet<EvalResult>,
}

#[derive(Clone, Debug)]
pub struct GenerationRunResult<EvalResult> {
    pub statuses: HashMap<EvalResult, usize>,
    pub new_codes: HashMap<EvalResult, StdinSample>,
}

pub type DynEval<EvalResult, Err> =
    Box<dyn Evaluator<Item = StdinSample, EvalResult = EvalResult, Error = Err> + 'static>;

pub type DynGen<EvalResult> = Box<dyn Generator<EvalResult, Item = StdinSample> + 'static>;

impl<Err, EvalResult> Fuzzer<EvalResult, Err>
where
    EvalResult: Sample,
    Err: std::error::Error,
{
    pub fn new(
        generator: Box<dyn Generator<EvalResult, Item = StdinSample> + 'static>,
        evaluator: Box<
            dyn Evaluator<Item = StdinSample, EvalResult = EvalResult, Error = Err> + 'static,
        >,
    ) -> Self {
        Fuzzer {
            sample_generator: generator,
            library: vec![],
            evaluator,
            unique_crashes: HashSet::new(),
        }
    }

    pub fn add_to_library(&mut self, sample: StdinSample) -> Result<(), Err> {
        let scored = self.evaluator.score(sample)?;
        self.library.push(scored);
        Ok(())
    }

    pub fn run_generation(&mut self) -> Result<GenerationRunResult<EvalResult>, Err> {
        let mut library = vec![];

        std::mem::swap(&mut library, &mut self.library);

        let (mut keep, new_samples) = self.sample_generator.generate_samples(library);

        let mut scored = new_samples
            .into_iter()
            .map(|sample| self.evaluator.score(sample))
            .collect::<Result<Vec<_>, Err>>()?;

        let mut stats: HashMap<EvalResult, usize> = HashMap::new();

        let mut new_results: HashMap<EvalResult, StdinSample> = Default::default();

        for item in &scored {
            *stats.entry(item.result.clone()).or_default() += 1;

            if !self.unique_crashes.contains(&item.result) {
                self.unique_crashes.insert(item.result.clone());
                new_results.insert(item.result.clone(), item.sample.clone());
            }
        }

        keep.append(&mut scored);

        self.library = keep;

        Ok(GenerationRunResult {
            statuses: stats,
            new_codes: new_results,
        })
    }
}
