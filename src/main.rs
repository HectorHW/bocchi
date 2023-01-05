use std::process;

use execution::{ExitCodeEvaluator, TraceEvaluator};
use fuzzing::Fuzzer;
use itertools::Itertools;
use ptracer::disable_aslr;
use sample_generation::{random, RandomMutator};

use crate::configuration::{load_config, ConfigReadError};

mod analysys;
mod configuration;
mod execution;
mod fuzzing;
mod sample_generation;

fn print_input(data: &[u8]) -> String {
    format!(
        "{} [{}]",
        data.iter().map(|digit| format!("{digit:02x}")).join(" "),
        String::from_utf8_lossy(data)
    )
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

    let mut fuzzer = Fuzzer::new(
        RandomMutator {
            generation_size: 1000,
            sample_len_limit: config.stdin.unwrap().limit,
        },
        TraceEvaluator::new(mapping),
    );

    match fuzzer.add_to_library(random(5)) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("error executing : {e:?}");
            process::exit(exitcode::NOPERM);
        }
    };

    let mut gen = 0;

    loop {
        gen += 1;
        let exec_status = match fuzzer.run_generation() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error executing : {e:?}");
                process::exit(exitcode::NOPERM);
            }
        };

        println!("running generation {gen}");

        for (new_code, sample) in exec_status.new_codes {
            println!("found new interesting sample");
            println!(
                "    run result: {} functions, exit: {}",
                new_code.trajectory.len(),
                new_code.result
            );
            println!("    sample: {}", print_input(&sample));
            println!("trajectory: {:?}", new_code.trajectory)
        }
    }
}
