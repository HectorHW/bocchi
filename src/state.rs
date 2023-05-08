use std::{
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Instant,
};

use crate::sample_library::VectorLibrary;

#[derive(Clone)]
pub struct State {
    pub tested_samples: usize,
    pub improvements: usize,
    pub total_crashes: usize,
    pub total_nonzero: usize,
    pub total_working: usize,

    pub start_time: Instant,
    pub last_unique_crash: Option<Instant>,
    pub last_new_path: Option<Instant>,
    pub executions: ringbuffer::AllocRingBuffer<Instant>,
}

impl State {
    pub fn new() -> Self {
        State {
            tested_samples: 0,
            improvements: 0,
            total_crashes: 0,
            total_nonzero: 0,
            total_working: 0,
            start_time: Instant::now(),
            last_unique_crash: None,
            last_new_path: None,
            executions: ringbuffer::AllocRingBuffer::with_capacity(512),
        }
    }
}

pub static mut FUZZER_RUNNNIG: AtomicBool = AtomicBool::new(true);

pub type AM<T> = Arc<Mutex<T>>;

pub type Library = VectorLibrary<crate::execution::RunTrace, crate::sample::Sample>;
