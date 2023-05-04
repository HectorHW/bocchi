use crate::sample_library::{ComparisonKey, CoverageScore, Library, SizeScore};

pub trait Mutator {
    type Item: Sized + Clone;
    type MutInfo;

    fn mutate_sample(
        &mut self,
        sample: Self::Item,
        library: &[Self::Item],
    ) -> (Self::Item, Self::MutInfo);

    fn update_scores(&mut self, index: Self::MutInfo, result: RunResult);
}

pub trait Evaluator {
    type Item: Sized + Clone;
    type EvalResult;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<TestedSample<Self::Item, Self::EvalResult>, anyhow::Error>;
}

#[derive(Clone, Debug)]
pub struct TestedSample<Sample, EvalResult> {
    pub sample: Sample,
    pub result: EvalResult,
}

impl<S, E: Eq> ComparisonKey for TestedSample<S, E> {
    type Key = E;

    fn get_key(&self) -> &Self::Key {
        &self.result
    }
}

impl<S, E: CoverageScore> CoverageScore for TestedSample<S, E> {
    fn get_score(&self) -> f64 {
        self.result.get_score()
    }
}

pub struct Fuzzer<MutInfo> {
    pub library: Box<dyn Library<Item = crate::sample::Sample, Key = crate::execution::RunTrace>>,
    mutator: Box<dyn Mutator<Item = crate::sample::Sample, MutInfo = MutInfo>>,
    evaluator:
        Box<dyn Evaluator<Item = crate::sample::Sample, EvalResult = crate::execution::RunTrace>>,
}

#[derive(Clone, Debug)]
pub enum RunResult {
    Nothing,
    New(crate::sample::Sample, crate::execution::RunTrace),
    SizeImprovement(crate::sample::Sample, crate::execution::RunTrace),
}

pub type DynEval<Sample, EvalResult> =
    Box<dyn Evaluator<Item = Sample, EvalResult = EvalResult> + 'static>;

impl<MutInfo> Fuzzer<MutInfo> {
    pub fn new(
        mutator: Box<dyn Mutator<Item = crate::sample::Sample, MutInfo = MutInfo> + 'static>,
        library: Box<
            dyn Library<Key = crate::execution::RunTrace, Item = crate::sample::Sample> + 'static,
        >,
        evaluator: Box<
            dyn Evaluator<Item = crate::sample::Sample, EvalResult = crate::execution::RunTrace>
                + 'static,
        >,
    ) -> Self {
        Fuzzer {
            mutator,
            library,
            evaluator,
        }
    }

    fn put_in_library(
        &mut self,
        tested: TestedSample<crate::sample::Sample, crate::execution::RunTrace>,
    ) -> RunResult {
        if let Some(existing) = self.library.find_existing(&tested.result) {
            if existing.get_size_score() > tested.sample.get_size_score() {
                self.library
                    .upsert(tested.result.clone(), tested.sample.clone());
                RunResult::SizeImprovement(tested.sample, tested.result)
            } else {
                RunResult::Nothing
            }
        } else {
            self.library
                .upsert(tested.result.clone(), tested.sample.clone());

            RunResult::New(tested.sample, tested.result)
        }
    }

    pub fn run_once(&mut self) -> Result<RunResult, anyhow::Error> {
        let sample = self.library.pick_random();

        let (mutated, mut_info) = self.mutator.mutate_sample(sample, self.library.linearize());

        let traced = self.evaluator.score(mutated)?;

        let result = self.put_in_library(traced);

        self.mutator.update_scores(mut_info, result.clone());

        Ok(result)
    }

    pub fn put_seed(&mut self, sample: crate::sample::Sample) -> Result<RunResult, anyhow::Error> {
        let traced = self.evaluator.score(sample)?;

        let result = self.put_in_library(traced);

        Ok(result)
    }
}
