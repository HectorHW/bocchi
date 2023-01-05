use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fmt::Display,
    io::Write,
    os::fd::AsRawFd,
    process::{self, Child, Command, Stdio},
};

use memfile::MemFile;
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

    type EvalResult = ExecResult;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<crate::fuzzing::SampleData<Self::Item, Self::EvalResult>, anyhow::Error> {
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

pub struct FunctionTracer<S: InputPassStyle> {
    binary: ElfInfo,
    pass_style: S,
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

fn pass_input<F: std::io::Write>(mut file: F, input: &[u8]) -> Result<F, std::io::Error> {
    file.write_all(input)?;
    file.flush()?;
    Ok(file)
}

pub trait InputPassStyle: Sized {
    fn make_command<P: AsRef<OsStr>>(exec_path: P, obj: &mut FunctionTracer<Self>) -> Command;

    fn get_file(obj: &mut FunctionTracer<Self>, tracer: &mut Ptracer) -> Box<dyn Write>;
}

pub struct PassViaStdin {}
pub struct PassViaFile {
    file: Option<MemFile>,
}

impl InputPassStyle for PassViaFile {
    fn make_command<P: AsRef<OsStr>>(
        exec_path: P,
        obj: &mut FunctionTracer<PassViaFile>,
    ) -> Command {
        let mut command = Command::new(exec_path);

        obj.pass_style.file =
            Some(MemFile::create_default("stdin").expect("failure creating memfile"));

        command
            .arg(format!(
                "/proc/{}/fd/{}",
                process::id(),
                obj.pass_style.file.as_ref().unwrap().as_raw_fd()
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        command
    }

    fn get_file(obj: &mut FunctionTracer<Self>, _tracer: &mut Ptracer) -> Box<dyn Write> {
        Box::new(obj.pass_style.file.take().unwrap())
    }
}
impl InputPassStyle for PassViaStdin {
    fn make_command<P: AsRef<OsStr>>(
        exec_path: P,
        _obj: &mut FunctionTracer<PassViaStdin>,
    ) -> Command {
        let mut command = Command::new(exec_path);

        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn get_file(_obj: &mut FunctionTracer<Self>, tracer: &mut Ptracer) -> Box<dyn Write> {
        Box::new(tracer.child_mut().stdin.take().unwrap())
    }
}

impl FunctionTracer<PassViaFile> {
    pub fn new(binary: ElfInfo) -> Self {
        Self {
            binary,
            pass_style: PassViaFile { file: None },
        }
    }
}

impl FunctionTracer<PassViaStdin> {
    pub fn new(binary: ElfInfo) -> Self {
        Self {
            binary,
            pass_style: PassViaStdin {},
        }
    }
}

impl<S: InputPassStyle> FunctionTracer<S> {
    fn set_breakpoints(&self, tracer: &mut Ptracer) -> Result<(), TraceError> {
        for function in &self.binary.functions {
            tracer.insert_breakpoint(self.binary.base_offset.unwrap() + function.offset)?;
        }
        Ok(())
    }

    pub fn run(&mut self, input: &[u8]) -> Result<RunTrace, TraceError> {
        let path = self.binary.path.clone();
        let cmd = S::make_command(path, self);

        let mut tracer = Ptracer::spawn(cmd, None)?;

        if self.binary.base_offset.is_none() {
            self.binary.base_offset = Some(determine_offset(tracer.child())?);
        }

        self.set_breakpoints(&mut tracer)?;

        let file = pass_input(S::get_file(self, &mut tracer), input)?;

        let mut trajectory = vec![];

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
            trajectory.push(adjusted_rip);
        }

        assert!(result.is_some(), "child did not finish executing");

        Ok(RunTrace {
            result: result.unwrap(),
            trajectory,
        })
    }
}

pub struct TraceEvaluator<S: InputPassStyle> {
    seen_errors: HashMap<RunTrace, Vec<u8>>,
    tracer: FunctionTracer<S>,
}

impl TraceEvaluator<PassViaFile> {
    pub fn new(info: ElfInfo) -> Self {
        Self {
            seen_errors: Default::default(),
            tracer: FunctionTracer::<PassViaFile>::new(info),
        }
    }
}

impl TraceEvaluator<PassViaStdin> {
    pub fn new(info: ElfInfo) -> Self {
        Self {
            seen_errors: Default::default(),
            tracer: FunctionTracer::<PassViaStdin>::new(info),
        }
    }
}

impl<S: InputPassStyle> Evaluator for TraceEvaluator<S> {
    type Item = Vec<u8>;

    type EvalResult = RunTrace;

    fn score(
        &mut self,
        sample: Self::Item,
    ) -> Result<SampleData<Self::Item, Self::EvalResult>, anyhow::Error> {
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
