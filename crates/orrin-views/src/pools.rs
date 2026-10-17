use std::sync::OnceLock;

use rayon::{ThreadPool, ThreadPoolBuilder};

pub fn files() -> &'static ThreadPool {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| pool("orrin-files", cores() * 3))
}

pub fn background() -> &'static ThreadPool {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| pool("orrin-spares", (cores() / 2).max(1)))
}

fn cores() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

fn pool(name: &'static str, threads: usize) -> ThreadPool {
    let built = ThreadPoolBuilder::new().num_threads(threads).thread_name(move |i| format!("{name}-{i}")).build();
    built.unwrap_or_else(|err| panic!("cannot start the {name} thread pool: {err}"))
}
