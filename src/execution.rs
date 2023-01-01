use std::{collections::HashSet, io::Write, process::Stdio};

use crate::fuzzing::{Evaluator, SampleData};

#[derive(Debug)]
pub enum ExecutionError {
    SpawnError(std::io::Error),
    StdinError(std::io::Error),
}

pub struct ExitCodeEvaluator {
    binary: String,
    seen_codes: HashSet<i32>,
}

impl ExitCodeEvaluator {
    pub fn new(binary: String) -> Self {
        ExitCodeEvaluator {
            binary,
            seen_codes: Default::default(),
        }
    }
}

impl Evaluator for ExitCodeEvaluator {
    type Item = Vec<u8>;
    type Error = ExecutionError;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<crate::fuzzing::SampleData<Self::Item>, Self::Error> {
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

        let produced_code = exec_result.status.code().unwrap_or(-1);

        Ok(if self.seen_codes.contains(&produced_code) {
            SampleData {
                sample,
                score: 0f64,
                return_code: produced_code,
            }
        } else {
            self.seen_codes.insert(produced_code);

            SampleData {
                sample,
                score: 1f64,
                return_code: produced_code,
            }
        })
    }
}
