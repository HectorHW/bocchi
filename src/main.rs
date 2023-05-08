use fuzz_thread::spawn_fuzzer;

use ptracer::disable_aslr;
use sample_library::VectorLibrary;
use state::{State, FUZZER_RUNNNIG};
use std::sync::{Arc, Mutex};

use std::process;
use ui::serve_ui;

use crate::configuration::{load_config, ConfigReadError};

mod analysys;
mod configuration;
mod execution;
mod flags;
mod fuzz_thread;
mod fuzzing;
mod grammar;
mod mutation;
mod sample;
mod sample_library;
mod ui;

mod log;
mod state;

pub(crate) use log::log;

fn main() {
    unsafe {
        disable_aslr();
    }

    ctrlc::set_handler(move || {
        println!("received Ctrl+C!");

        unsafe { FUZZER_RUNNNIG.store(false, std::sync::atomic::Ordering::SeqCst) };

        process::exit(exitcode::SOFTWARE);
    })
    .expect("Error setting Ctrl-C handler");

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

    let config = Box::leak(Box::new(config));

    let library = Arc::new(Mutex::new(VectorLibrary::new()));

    let state = Arc::new(Mutex::new(State::new()));

    let fuzzer_thread_handle = match spawn_fuzzer(config, library.clone(), state.clone()) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("error while spawning fuzzer thread: {e}");
            process::exit(exitcode::SOFTWARE);
        }
    };

    let ui_errors = serve_ui(library, state, config);

    unsafe { FUZZER_RUNNNIG.store(false, std::sync::atomic::Ordering::SeqCst) };

    let _ = fuzzer_thread_handle.join().map_err(|e| {
        eprintln!("error inside fuzzing thread: {e:?}");
        process::exit(exitcode::SOFTWARE)
    });

    match ui_errors {
        Ok(_) => {}
        Err(e) => {
            eprintln!("error in ui: {e}");
            process::exit(exitcode::SOFTWARE)
        }
    }
}
