//! Hold a fixed buffer so memory-limit tests can cross a cgroup ceiling.
//!
//! Subcommands match the shapes those tests need: one large buffer, a silent
//! buffer, a delayed buffer, a slow ramp, and a buffer that should survive
//! when no limit is configured.

use std::io::{self, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn println_flush(line: &str) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn allocate_mib(mib: usize) -> Vec<u8> {
    let bytes = mib.saturating_mul(1024 * 1024);
    let mut data = vec![1u8; bytes];
    if bytes > 0 {
        data[0] = 2;
        data[bytes - 1] = 3;
    }
    std::hint::black_box(data.len());
    data
}

fn burst() {
    println_flush("Allocating 128 MiB...");
    let data = allocate_mib(128);
    std::mem::forget(data);
    println_flush("Allocation succeeded (should not reach here)");
}

fn silent() {
    let data = allocate_mib(128);
    std::mem::forget(data);
}

fn background() {
    println_flush("BG: allocating...");
    thread::sleep(Duration::from_millis(500));
    let data = allocate_mib(128);
    std::mem::forget(data);
    println_flush("BG: done (should not reach here)");
    thread::sleep(Duration::from_secs(60));
}

fn gradual() {
    let mut chunks = Vec::with_capacity(128);
    for i in 0..128 {
        chunks.push(allocate_mib(1));
        println_flush(&format!("Allocated {} MiB", i + 1));
        thread::sleep(Duration::from_millis(50));
    }
    std::hint::black_box(chunks.len());
    println_flush("Finished all allocations (should not reach here)");
    std::mem::forget(chunks);
}

fn plain() {
    let data = allocate_mib(64);
    std::mem::forget(data);
    println_flush("Allocated 64 MiB without limits");
}

fn main() -> ExitCode {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "burst" => burst(),
        "silent" => silent(),
        "background" => background(),
        "gradual" => gradual(),
        "plain" => plain(),
        _ => {
            eprintln!("usage: cgroup_memory_alloc burst|silent|background|gradual|plain");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
