use std::sync::Mutex;

use chrono::{Datelike, Local, Timelike};
use itertools::Itertools;
use lazy_static::lazy_static;
use ringbuffer::RingBufferWrite;
use ringbuffer::{AllocRingBuffer, RingBufferExt};

lazy_static! {
    static ref BUFFER: Mutex<AllocRingBuffer<String>> =
        Mutex::new(AllocRingBuffer::with_capacity(128));
}

pub fn write_message(message: &str) {
    let time = Local::now();

    let human_readable = format!(
        "{:02}.{:02} {:02}:{:02}:{:02}",
        time.day(),
        time.month(),
        time.hour(),
        time.minute(),
        time.second()
    );

    let mut buffer = BUFFER.lock().unwrap();

    buffer.push(format!("[{human_readable}] {message}"))
}

macro_rules! log{
    ($($e:expr),+) => {
        crate::log::write_message(&format!($($e),+))
    }
}

pub(crate) use log;

pub fn pull_messages(n: usize) -> Vec<String> {
    let mut items = {
        let buffer = BUFFER.lock().unwrap();

        buffer.iter().rev().take(n).map(Clone::clone).collect_vec()
    };
    items.reverse();
    items
}
