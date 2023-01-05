use std::{collections::HashSet, fmt::Display, io::Write, process::Stdio};

use crate::fuzzing::{Evaluator, SampleData};

#[derive(Debug)]
pub enum ExecutionError {
    SpawnError(std::io::Error),
    StdinError(std::io::Error),
}

pub struct ExitCodeEvaluator {
    binary: String,
    seen_codes: HashSet<ExecResult>,
}

impl ExitCodeEvaluator {
    pub fn new(binary: String) -> Self {
        ExitCodeEvaluator {
            binary,
            seen_codes: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ExecResult {
    Code(i32),
    Signal,
}

impl Display for ExecResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecResult::Code(code) => write!(f, "code {code}"),
            ExecResult::Signal => write!(f, "killed"),
        }
    }
}

impl Evaluator for ExitCodeEvaluator {
    type Item = Vec<u8>;
    type Error = ExecutionError;

    type EvalResult = ExecResult;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<crate::fuzzing::SampleData<Self::Item, Self::EvalResult>, Self::Error> {
        let mut process = std::process::Command::new(&self.binary)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .map_err(ExecutionError::SpawnError)?;

        {
            let mut child_stdin = process.stdin.take().unwrap();

            child_stdin
                .write_all(&sample)
                .map_err(ExecutionError::StdinError)?;
        }

        let exec_result = process.wait_with_output().unwrap();

        let result = exec_result
            .status
            .code()
            .map(ExecResult::Code)
            .unwrap_or(ExecResult::Signal);

        Ok(if self.seen_codes.contains(&result) {
            SampleData {
                sample,
                score: 0f64,
                result,
            }
        } else {
            self.seen_codes.insert(result.clone());

            SampleData {
                sample,
                score: 1f64,
                result,
            }
        })
    }
}
