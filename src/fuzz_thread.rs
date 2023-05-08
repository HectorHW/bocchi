use std::{
    path::PathBuf,
    process,
    thread::{self, JoinHandle},
    time::Instant,
};

use anyhow::{anyhow, Context};
use rand::Rng;
use ringbuffer::RingBufferWrite;

use crate::{
    analysys,
    configuration::FuzzConfig,
    execution::{self},
    fuzzing::Fuzzer,
    grammar::{generation::TreeNodeItem, Grammar, TreeNode},
    mutation::build_mutator,
    sample::Sample,
    sample_library::Library as LibT,
    state::{Library, State, AM, FUZZER_RUNNNIG},
};

fn get_unique_name() -> String {
    let mut rng = rand::thread_rng();

    (0..6).map(|_| format!("{:x}", rng.gen::<u8>())).collect()
}

fn get_crash_path(config: &'static FuzzConfig, name: &str) -> PathBuf {
    PathBuf::from(&config.output.directory).join(name)
}

fn save_crash(sample: &crate::sample::Sample, path: PathBuf) -> Result<(), std::io::Error> {
    let dir = {
        let mut path = path.clone();

        path.pop();

        path
    };

    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, &sample.result)
}

pub fn spawn_fuzzer(
    config: &'static FuzzConfig,
    library: AM<Library>,
    state: AM<State>,
) -> Result<JoinHandle<Result<(), std::io::Error>>, anyhow::Error> {
    let path = config.binary.path.clone();

    let mapping = match analysys::analyze_binary(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error analyzing binary for trace evaluator");
            eprintln!("error: {e}");
            process::exit(exitcode::DATAERR)
        }
    };

    crate::log!(
        "extracted {} functions from executable",
        mapping.functions.len()
    );

    let (seeds, grammar) = match &config.input {
        crate::configuration::InputOptions::Grammar { grammar } => {
            crate::log!("fuzzer started in grammar mode");

            let grammar_content = match std::fs::read_to_string(grammar) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("error reading grammar file: {e}");
                    process::exit(exitcode::IOERR);
                }
            };

            let grammar = match crate::grammar::parse_grammar(&grammar_content) {
                Ok(grammar) => grammar,
                Err(e) => {
                    eprintln!("errors while parsing grammar");
                    eprintln!("{e}");
                    process::exit(exitcode::CONFIG)
                }
            };

            let depth_limit = 30;

            let generator =
                crate::grammar::generation::Generator::new(grammar.clone(), depth_limit);

            let initial = generator.generate();

            crate::log!("generated initial sample of size {}", initial.folded.len());

            (vec![crate::sample::Sample::new(initial, vec![])], grammar)
        }
        crate::configuration::InputOptions::Seeds { seeds: s } => {
            crate::log!("fuzzer started in binary mode");

            let mut seeds = vec![];

            for subitem in std::fs::read_dir(s).context("reading seeds directory")? {
                let dir_entry = subitem?;

                let content = std::fs::read(dir_entry.path()).with_context(|| {
                    format!(
                        "while reading seed at {}",
                        dir_entry.path().as_os_str().to_string_lossy()
                    )
                })?;

                let root = TreeNodeItem::HexString(content);
                let tree: TreeNode = root.into();
                let folded_tree = tree.fold_into_sample();

                let sample = Sample::new(folded_tree, vec![]);

                seeds.push(sample);
            }

            if seeds.is_empty() {
                return Err(anyhow!(
                    "got zero samples after looking in configured seeds directory"
                ));
            }

            crate::log!("loaded {} seed(s) from {}", seeds.len(), s);

            (seeds, Grammar::empty())
        }
    };

    Ok(thread::spawn(move || {
        let mutator = build_mutator(config, &grammar);

        let evaluator = execution::TraceEvaluator::new(mapping, config.binary.pass_style);
        let mut fuzzer = Fuzzer::new(mutator, library.clone(), evaluator);

        for seed in seeds {
            fuzzer.put_seed(seed).unwrap();
        }

        while unsafe { FUZZER_RUNNNIG.load(std::sync::atomic::Ordering::SeqCst) } {
            let result = match fuzzer.run_once() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error executing : {e:?}");
                    panic!("encuntered error in fuzzer: {e:?}");
                }
            };

            let mut library = library.lock().unwrap();
            let mut state = state.lock().unwrap();

            state.tested_samples += 1;
            state.executions.push(Instant::now());

            match result.status {
                crate::fuzzing::RunResultStatus::Nothing => {}
                crate::fuzzing::RunResultStatus::New => {
                    state.last_new_path = Some(Instant::now());
                    if let execution::ExecResult::Signal = result.trace.result {
                        state.last_unique_crash = Some(Instant::now());

                        let name = get_unique_name();

                        let path = get_crash_path(config, &name);

                        library.add_name(&result.trace, name.clone());

                        save_crash(&result.sample, path.clone())?;
                        crate::log!(
                            "found new crash and saved it as {}",
                            path.into_os_string().into_string().unwrap()
                        );
                    }
                }
                crate::fuzzing::RunResultStatus::SizeImprovement => {
                    state.improvements += 1;

                    if let execution::ExecResult::Signal = result.trace.result {
                        let name = library
                            .find_existing(&result.trace)
                            .as_ref()
                            .unwrap()
                            .unique_name
                            .as_ref()
                            .unwrap()
                            .clone();

                        let path = get_crash_path(config, &name);

                        save_crash(&result.sample, path.clone())?;
                        crate::log!(
                            "found smaller example for crash {}",
                            path.into_os_string().into_string().unwrap()
                        );
                    }
                }
            }

            match result.trace.result {
                execution::ExecResult::Code(0) => state.total_working += 1,
                execution::ExecResult::Code(_) => state.total_nonzero += 1,
                execution::ExecResult::Signal => {
                    state.total_crashes += 1;
                }
            }
        }

        Ok(())
    }))
}
