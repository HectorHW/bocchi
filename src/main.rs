use std::process;

use execution::ExitCodeEvaluator;
use fuzzing::Fuzzer;
use itertools::Itertools;
use sample_generation::{random, RandomMutator};

use crate::configuration::{load_config, ConfigReadError};

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
            eprintln!("{e}");
            process::exit(exitcode::IOERR)
        }

        Err(ConfigReadError::ParseError(e)) => {
            eprintln!("{e}");
            process::exit(exitcode::CONFIG)
        }
    };

    let path = config.binary.path.clone();

    let mut fuzzer = Fuzzer::new(
        RandomMutator {
            generation_size: 1000,
            sample_len_limit: config.stdin.unwrap().limit,
        },
        ExitCodeEvaluator::new(path),
    );

    match fuzzer.add_to_library(random(5)) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("error executing : {e:?}");
            process::exit(exitcode::NOPERM);
        }
    };

    loop {
        let exec_status = match fuzzer.run_generation() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error executing : {e:?}");
                process::exit(exitcode::NOPERM);
            }
        };

        println!("generation result:");
        for (&code, count) in &exec_status.statuses {
            println!("{code: >7}: {count}");
        }

        for (new_code, sample) in exec_status.new_codes {
            println!("found new interesting sample");
            println!("    code: {new_code}");
            println!("    sample: {}", print_input(&sample));
        }
    }
}
