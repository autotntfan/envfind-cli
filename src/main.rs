use std::collections::VecDeque;
use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;

use envfind::cli::{CommandLine, parse_command};
use envfind::discovery::{default_providers, discover_all};
use envfind::model::{Candidate, ProbeResult};
use envfind::output::render_table;
use envfind::probe::{PROBE_TIMEOUT, probe};

pub fn probe_candidates(candidates: Vec<Candidate>, query: &str) -> Vec<(Candidate, ProbeResult)> {
    probe_candidates_with(candidates, query, |python, query| {
        probe(python, query, PROBE_TIMEOUT)
    })
}

pub fn probe_candidates_with<F>(
    candidates: Vec<Candidate>,
    query: &str,
    probe_fn: F,
) -> Vec<(Candidate, ProbeResult)>
where
    F: Fn(&Path, &str) -> Option<ProbeResult> + Send + Sync + 'static,
{
    if candidates.is_empty() {
        return Vec::new();
    }
    let cpus = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let workers = cpus.clamp(1, 8).min(candidates.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(candidates)));
    let results = Arc::new(Mutex::new(Vec::new()));
    let probe_fn = Arc::new(probe_fn);
    let mut handles = Vec::new();
    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        let probe_fn = Arc::clone(&probe_fn);
        let query = query.to_owned();
        handles.push(thread::spawn(move || {
            loop {
                let candidate = queue.lock().ok().and_then(|mut q| q.pop_front());
                let Some(candidate) = candidate else { break };
                if let Some(result) = probe_fn(&candidate.python_path, &query) {
                    if (result.import_match || result.distribution_match)
                        && let Ok(mut out) = results.lock()
                    {
                        out.push((candidate, result));
                    }
                }
            }
        }));
    }
    for handle in handles {
        let _ = handle.join();
    }
    Arc::try_unwrap(results)
        .ok()
        .and_then(|m| m.into_inner().ok())
        .unwrap_or_default()
}

fn usage() {
    println!(
        "Usage: envfind <name>\n\nFind managed Python environments that match an import or installed distribution name.\n\nOptions:\n  -h, --help       Show help\n      --version    Show version"
    );
}

fn main() -> ExitCode {
    match parse_command(env::args()) {
        Ok(CommandLine::Help) => {
            usage();
            ExitCode::SUCCESS
        }
        Ok(CommandLine::Version) => {
            println!("envfind {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(CommandLine::Run(cli)) => {
            let providers = default_providers();
            let candidates = discover_all(&providers);
            let matches = probe_candidates(candidates, &cli.query);
            if matches.is_empty() {
                println!("No matching Python environment found.");
                ExitCode::from(1)
            } else {
                print!("{}", render_table(&matches));
                ExitCode::SUCCESS
            }
        }
        Err(_) => {
            eprintln!("Usage: envfind <name>");
            ExitCode::from(2)
        }
    }
}
