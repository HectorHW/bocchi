use std::{
    process,
    thread::{self, JoinHandle},
    time::Instant,
};

use ringbuffer::RingBufferWrite;

use crate::{
    analysys,
    configuration::FuzzConfig,
    execution::{self},
    fuzzing::Fuzzer,
    mutation::build_mutator,
    state::{Library, State, AM, FUZZER_RUNNNIG},
};

pub fn spawn_fuzzer(
    config: &'static FuzzConfig,
    library: AM<Library>,
    state: AM<State>,
) -> Result<JoinHandle<()>, anyhow::Error> {
    let grammar_content = match std::fs::read_to_string(&config.grammar.path) {
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

    let generator = crate::grammar::generation::Generator::new(grammar.clone(), depth_limit);

    let initial = generator.generate();

    let seed = crate::sample::Sample::new(initial.clone(), vec![]);

    crate::log!("generated initial sample of size {}", initial.folded.len());

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

    Ok(thread::spawn(move || {
        let mutator = build_mutator(config, &grammar);

        let evaluator = execution::TraceEvaluator::new(mapping, config.stdin.pass_style);
        let mut fuzzer = Fuzzer::new(mutator, library, evaluator);

        fuzzer.put_seed(seed).unwrap();

        while unsafe { FUZZER_RUNNNIG.load(std::sync::atomic::Ordering::SeqCst) } {
            let result = match fuzzer.run_once() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error executing : {e:?}");
                    panic!("encuntered error in fuzzer: {e:?}");
                }
            };

            let mut state = state.lock().unwrap();

            state.tested_samples += 1;
            state.executions.push(Instant::now());

            match result.status {
                crate::fuzzing::RunResultStatus::Nothing => {}
                crate::fuzzing::RunResultStatus::New => {
                    state.last_new_path = Some(Instant::now());
                    if let execution::ExecResult::Signal = result.trace.result {
                        state.last_unique_crash = Some(Instant::now());
                        crate::log!("found new crash");
                    }
                }
                crate::fuzzing::RunResultStatus::SizeImprovement => {
                    state.improvements += 1;
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
    }))
}
