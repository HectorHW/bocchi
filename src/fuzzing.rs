use std::sync::{Arc, Mutex};

use crate::{
    execution::{self},
    sample_library::{CoverageScore, Library, SizeScore},
};

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

    fn trace_detailed(
        &mut self,
        sample: Self::Item,
    ) -> Result<execution::DetailedTrace, anyhow::Error>;
}

#[derive(Clone, Debug)]
pub struct TestedSample<Sample, EvalResult> {
    pub sample: Sample,
    pub result: EvalResult,
}

impl<S, E: CoverageScore> CoverageScore for TestedSample<S, E> {
    fn get_score(&self) -> f64 {
        self.result.get_score()
    }
}
type AM<T> = Arc<Mutex<T>>;

pub struct Fuzzer<Lib, Mut, Eval, MutInfo>
where
    Lib: Library,
    Mut: Mutator<Item = crate::sample::Sample, MutInfo = MutInfo>,
    Eval: Evaluator<Item = crate::sample::Sample, EvalResult = crate::execution::RunTrace>,
{
    pub library: AM<Lib>,
    mutator: Mut,
    evaluator: Eval,
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub sample: crate::sample::Sample,
    pub trace: crate::execution::RunTrace,
    pub status: RunResultStatus,
}

#[derive(Clone, Debug)]
pub enum RunResultStatus {
    Nothing,
    New,
    SizeImprovement,
}

impl<Lib, Mut, Eval, MutInfo> Fuzzer<Lib, Mut, Eval, MutInfo>
where
    Lib: Library<Key = crate::execution::RunTrace, Item = crate::sample::Sample>,
    Mut: Mutator<Item = crate::sample::Sample, MutInfo = MutInfo>,
    Eval: Evaluator<Item = crate::sample::Sample, EvalResult = crate::execution::RunTrace>,
{
    pub fn new(mutator: Mut, library: AM<Lib>, evaluator: Eval) -> Self {
        Fuzzer {
            mutator,
            library,
            evaluator,
        }
    }

    fn put_in_library(
        &mut self,
        tested: TestedSample<crate::sample::Sample, crate::execution::RunTrace>,
    ) -> Result<RunResult, anyhow::Error> {
        let status = {
            let mut library = self.library.lock().unwrap();

            if let Some(existing) = library.find_existing(&tested.result) {
                if existing.item.get_size_score() > tested.sample.get_size_score() {
                    library.upsert(tested.result.clone(), tested.sample.clone());
                    RunResultStatus::SizeImprovement
                } else {
                    RunResultStatus::Nothing
                }
            } else {
                library.upsert(tested.result.clone(), tested.sample.clone());

                RunResultStatus::New
            }
        };

        Ok(RunResult {
            sample: tested.sample,
            trace: tested.result,
            status,
        })
    }

    pub fn run_once(&mut self) -> Result<RunResult, anyhow::Error> {
        let (mutated, mut_info) = {
            let mut library = self.library.lock().unwrap();

            let sample = library.pick_random();

            self.mutator.mutate_sample(sample, library.linearize())
        };

        let traced = self.evaluator.score(mutated)?;

        let result = self.put_in_library(traced)?;

        self.mutator.update_scores(mut_info, result.clone());

        Ok(result)
    }

    pub fn put_seed(&mut self, sample: crate::sample::Sample) -> Result<RunResult, anyhow::Error> {
        let traced = self.evaluator.score(sample)?;

        let result = self.put_in_library(traced)?;

        Ok(result)
    }
}
