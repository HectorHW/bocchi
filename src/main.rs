use configuration::PassStyle;
use execution::{PassViaFile, PassViaStdin, RunTrace};
use fuzzing::{DynEval, Fuzzer};
use itertools::Itertools;
use ptracer::disable_aslr;
use sample_generation::{random, RandomMutator};
use seeding::read_seeds;
use std::process;

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

mod seeding;

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

    let generator = Box::new(RandomMutator {
        generation_size: config.generation.population,
        sample_len_limit: config.generation.sample_limit,
    });

    let evaluator: DynEval<_> = if config.stdin.pass_style == PassStyle::File {
        Box::new(execution::TraceEvaluator::<PassViaFile>::new(mapping))
    } else {
        Box::new(execution::TraceEvaluator::<PassViaStdin>::new(mapping))
    };
    let mut fuzzer = Fuzzer::new(generator, evaluator);

    let seeds = if let Some(dir) = config.seeds.path {
        match read_seeds(&dir) {
            Ok(s) => {
                println!("read {} seeds", s.len());
                s
            }
            Err(e) => {
                eprintln!("failed to read seeds directory: {e}");
                process::exit(exitcode::DATAERR);
            }
        }
    } else {
        println!("no seed dir provided, seeding with single entry of five random bytes");
        vec![random(5)]
    };

    for seed in seeds {
        match fuzzer.add_to_library(seed) {
            Ok(Some(new)) => report_run(new),
            Ok(None) => {}
            Err(e) => {
                eprintln!("error executing : {e:?}");
                process::exit(exitcode::NOPERM);
            }
        };
    }

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

        for (new_code, _sample) in exec_status.new_codes {
            report_run(new_code)
        }
    }
}
