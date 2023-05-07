use std::{
    collections::HashMap,
    fmt::Display,
    io::Write,
    os::fd::AsRawFd,
    path::PathBuf,
    process::{self, Child, Command, Stdio},
};

use memfile::MemFile;
use ptracer::{nix::sys::wait::WaitStatus, Ptracer};

use crate::{
    analysys::ElfInfo,
    configuration::PassStyle as PassStyleCfg,
    fuzzing::{Evaluator, TestedSample},
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
}

impl ExitCodeEvaluator {
    pub fn new(binary: String) -> Self {
        ExitCodeEvaluator { binary }
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

    type EvalResult = ExecResult;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<crate::fuzzing::TestedSample<Self::Item, Self::EvalResult>, anyhow::Error> {
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

        Ok(TestedSample { sample, result })
    }
}

pub struct FunctionTracer {
    binary: ElfInfo,
    pass_style: InputPassStyle,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum Hits {
    #[default]
    Once,
    Twice,
    Many,
}

impl Hits {
    pub fn inc(self) -> Self {
        match self {
            Hits::Once => Hits::Twice,
            Hits::Twice => Hits::Many,
            Hits::Many => Hits::Many,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTrace {
    pub result: ExecResult,
    pub trajectory: HashMap<usize, Hits>,
}

impl crate::sample_library::CoverageScore for RunTrace {
    fn get_score(&self) -> f64 {
        self.trajectory.len() as f64 + 0.1
    }
}

impl crate::sample_library::ComparisonKey for RunTrace {
    type Key = RunTrace;

    fn get_key(&self) -> &Self::Key {
        self
    }
}

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

pub enum InputPassStyle {
    File(Option<MemFile>),
    StdIn,
}

impl FunctionTracer {
    pub fn new(binary: ElfInfo, pass_style: PassStyleCfg) -> Self {
        Self {
            binary,
            pass_style: if pass_style == PassStyleCfg::Stdin {
                InputPassStyle::StdIn
            } else {
                InputPassStyle::File(None)
            },
        }
    }
}

impl FunctionTracer {
    fn set_breakpoints(&self, tracer: &mut Ptracer) -> Result<(), TraceError> {
        for function in &self.binary.functions {
            tracer.insert_breakpoint(self.binary.base_offset.unwrap() + function.offset)?;
        }
        Ok(())
    }

    fn make_command(&mut self, path: PathBuf) -> Command {
        match &mut self.pass_style {
            InputPassStyle::StdIn => {
                let mut command = Command::new(path);

                command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                command
            }
            InputPassStyle::File(ref mut handle) => {
                let mut command = Command::new(path);

                let file =
                    Some(MemFile::create_default("stdin").expect("failure creating memfile"));

                command
                    .arg(format!(
                        "/proc/{}/fd/{}",
                        process::id(),
                        file.as_ref().unwrap().as_raw_fd()
                    ))
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                *handle = file;

                command
            }
        }
    }

    fn pass_input(
        &mut self,
        tracer: &mut Ptracer,
        input: &[u8],
    ) -> Result<Option<MemFile>, std::io::Error> {
        match &mut self.pass_style {
            InputPassStyle::File(f) => {
                let mut memfile = f.take().unwrap();

                memfile.write_all(input)?;
                memfile.flush()?;

                Ok(Some(memfile))
            }
            InputPassStyle::StdIn => {
                let mut stdin = tracer.child_mut().stdin.take().unwrap();

                stdin.write_all(input)?;
                stdin.flush()?;

                Ok(None)
            }
        }
    }

    pub fn run(&mut self, input: &[u8]) -> Result<RunTrace, TraceError> {
        let path = self.binary.path.clone();
        let cmd = self.make_command(path);

        let mut tracer = Ptracer::spawn(cmd, None)?;

        if self.binary.base_offset.is_none() {
            self.binary.base_offset = Some(determine_offset(tracer.child())?);
        }

        self.set_breakpoints(&mut tracer)?;

        let _maybe_needs_hold = self.pass_input(&mut tracer, input)?;

        let mut trajectory: HashMap<usize, Hits> = Default::default();

        let mut result = None;

        while tracer.cont(ptracer::ContinueMode::Default).is_ok() {
            match tracer.event() {
                WaitStatus::Exited(_pid, code) => result = Some(ExecResult::Code(*code)),
                WaitStatus::Signaled(_pid, _signal, _coredump) => {
                    result = Some(ExecResult::Signal);
                }
                _ => {}
            }
            let adjusted_rip = tracer.registers().rip as usize - self.binary.base_offset.unwrap();
            let new_value = *trajectory
                .entry(adjusted_rip)
                .and_modify(|k| *k = k.inc())
                .or_default();

            if matches!(new_value, Hits::Many) {
                tracer
                    .remove_breakpoint(tracer.registers().rip as usize)
                    .unwrap();
            }
        }

        assert!(result.is_some(), "child did not finish executing");

        Ok(RunTrace {
            result: result.unwrap(),
            trajectory,
        })
    }
}

pub struct TraceEvaluator {
    tracer: FunctionTracer,
}

impl TraceEvaluator {
    pub fn new(info: ElfInfo, pass_style: PassStyleCfg) -> Self {
        Self {
            tracer: FunctionTracer::new(info, pass_style),
        }
    }
}

impl Evaluator for TraceEvaluator {
    type Item = crate::sample::Sample;

    type EvalResult = RunTrace;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<TestedSample<Self::Item, Self::EvalResult>, anyhow::Error> {
        let result = self.tracer.run(sample.get_folded())?;

        Ok(TestedSample { sample, result })
    }
}
