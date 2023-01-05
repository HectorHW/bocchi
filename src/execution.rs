use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    io::Write,
    process::{Child, Command, Stdio},
};

use ptracer::{nix::sys::wait::WaitStatus, Ptracer};

use crate::{
    analysys::ElfInfo,
    fuzzing::{Evaluator, SampleData},
};

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("error spawning child: {0}")]
    SpawnError(std::io::Error),
    #[error("error communicating with child child: {0}")]
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

pub struct FunctionTracer {
    binary: ElfInfo,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RunTrace {
    pub result: ExecResult,
    pub trajectory: Vec<usize>,
}

impl crate::fuzzing::Sample for RunTrace {}

#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error(transparent)]
    Spawn(#[from] ptracer::TracerError),

    #[error("error accessing child process: {0}")]
    IO(#[from] std::io::Error),

    #[error("error working with breakpoints: {0}")]
    Nix(#[from] ptracer::nix::Error),
}

fn determine_offset(child: &Child) -> std::io::Result<usize> {
    let pid = child.id();
    let maps = proc_maps::get_process_maps(pid as proc_maps::linux_maps::Pid)?;
    Ok(maps[0].start())
}

fn pass_stdin(child: &mut Child, input: &[u8]) -> Result<(), std::io::Error> {
    let mut stdin = child.stdin.take().unwrap();

    stdin.write_all(input)
}

impl FunctionTracer {
    pub fn new(binary: ElfInfo) -> Self {
        Self { binary }
    }

    fn make_command(&self) -> Command {
        let mut command = Command::new(&self.binary.path);

        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn set_breakpoints(&self, tracer: &mut Ptracer) -> Result<(), TraceError> {
        for function in &self.binary.functions {
            tracer.insert_breakpoint(self.binary.base_offset.unwrap() + function.offset)?;
        }
        Ok(())
    }

    pub fn run(&mut self, input: &[u8]) -> Result<RunTrace, TraceError> {
        let mut tracer = Ptracer::spawn(self.make_command(), None)?;

        if self.binary.base_offset.is_none() {
            self.binary.base_offset = Some(determine_offset(tracer.child())?);
        }

        self.set_breakpoints(&mut tracer)?;

        pass_stdin(tracer.child_mut(), input)?;

        let mut trajectory = vec![];

        let mut result = None;

        while tracer.cont(ptracer::ContinueMode::Default).is_ok() {
            match tracer.event() {
                WaitStatus::Exited(_pid, code) => result = Some(ExecResult::Code(*code)),
                WaitStatus::Signaled(_pid, signal, _coredump) => {
                    result = Some(ExecResult::Signal);
                }
                _ => {}
            }
            let adjusted_rip = tracer.registers().rip as usize - self.binary.base_offset.unwrap();
            trajectory.push(adjusted_rip);
        }

        assert!(result.is_some(), "child did not finish executing");

        Ok(RunTrace {
            result: result.unwrap(),
            trajectory,
        })
    }
}

pub struct TraceEvaluator {
    seen_errors: HashMap<RunTrace, Vec<u8>>,
    tracer: FunctionTracer,
}

impl TraceEvaluator {
    pub fn new(info: ElfInfo) -> Self {
        Self {
            seen_errors: Default::default(),
            tracer: FunctionTracer::new(info),
        }
    }
}

impl Evaluator for TraceEvaluator {
    type Item = Vec<u8>;

    type EvalResult = RunTrace;

    type Error = TraceError;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<SampleData<Self::Item, Self::EvalResult>, Self::Error> {
        let result = self.tracer.run(&sample)?;

        Ok(if self.seen_errors.contains_key(&result) {
            SampleData {
                sample,
                score: result.trajectory.len() as f64 * 0.1,
                result,
            }
        } else {
            self.seen_errors.insert(result.clone(), sample.clone());

            SampleData {
                sample,
                score: result.trajectory.len() as f64 + 100f64,
                result,
            }
        })
    }
}
