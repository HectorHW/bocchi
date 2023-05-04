use configuration::PassStyle;
use execution::{PassViaFile, PassViaStdin, RunTrace};
use fuzzing::{DynEval, Fuzzer};
use mutation::tree_level::TreeRegrow;
use ptracer::disable_aslr;
use sample_library::{Library, VectorLibrary};
use std::process;

use crate::configuration::{load_config, ConfigReadError};

mod analysys;
mod configuration;
mod execution;
mod flags;
mod fuzzing;
mod grammar;
mod mutation;
mod sample;
mod sample_library;

use crate::mutation::{build_mutator, MutateTree};

fn report_run(new_code: RunTrace) {
    println!("found new interesting sample");
    println!(
        "    run result: {} functions, exit: {}",
        new_code.trajectory.len(),
        new_code.result
    );
    //println!("    sample: {}", print_input(&sample));

    //println!("trajectory: {:?}", new_code.trajectory)
}

fn main() {
    let config = match load_config("fuzz.toml") {
        Ok(config) => config,
        Err(ConfigReadError::ReadError(e)) => {
            eprintln!("failed to read fuzz.toml: {e}");
            process::exit(exitcode::IOERR)
        }

        Err(ConfigReadError::ParseError(e)) => {
            eprintln!("{e}");
            process::exit(exitcode::CONFIG)
        }
    };

    let mut library: Box<
        dyn Library<Key = crate::execution::RunTrace, Item = crate::sample::Sample>,
    > = Box::new(VectorLibrary::new());

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

    println!("initial: {}", String::from_utf8_lossy(&initial.folded));

    let path = config.binary.path.clone();

    let mapping = match analysys::analyze_binary(&path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error analyzing binary for trace evaluator");
            eprintln!("error: {e}");
            process::exit(exitcode::DATAERR)
        }
    };

    unsafe {
        disable_aslr();
    }

    let mutator = build_mutator(&config, &grammar);

    let evaluator: DynEval<_, _> = if config.stdin.pass_style == PassStyle::File {
        Box::new(execution::TraceEvaluator::<PassViaFile>::new(mapping))
    } else {
        Box::new(execution::TraceEvaluator::<PassViaStdin>::new(mapping))
    };
    let mut fuzzer = Fuzzer::new(mutator, library, evaluator);

    println!("{:?}", fuzzer.put_seed(seed).unwrap());

    let mut gen = 0;

    loop {
        gen += 1;
        let exec_status = match fuzzer.run_once() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error executing : {e:?}");
                process::exit(exitcode::NOPERM);
            }
        };

        if gen % 100 == 0 {
            println!("at gen {gen}");
            println!("library size: {}", fuzzer.library.linearize().len())
        }

        match exec_status {
            fuzzing::RunResult::Nothing => {}
            fuzzing::RunResult::New(s, trace) => {
                println!(
                    "found new sample: {}",
                    String::from_utf8_lossy(s.get_folded())
                )
            }
            fuzzing::RunResult::SizeImprovement(s, trace) => {
                println!(
                    "improved size of sample: {}",
                    String::from_utf8_lossy(s.get_folded())
                )
            }
        }
    }
}
