use std::process;

use drail::cli;

fn main() {
    configure_thread_pools();

    if let Err(error) = cli::run() {
        if matches!(error, drail::error::DrailError::AlreadyReported { .. }) {
            process::exit(error.exit_code());
        }
        eprintln!("{error}");
        process::exit(error.exit_code());
    }
}

fn configure_thread_pools() {
    let num_threads = std::env::var("DRAIL_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(4, |n| (n.get() / 2).clamp(2, 6))
        });

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .ok();
}
