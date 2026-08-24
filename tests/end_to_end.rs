use envfind::model::{Candidate, Manager, ProbeMode, ProbeResult};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;
#[allow(dead_code)]
#[path = "../src/main.rs"]
mod app;
#[test]
fn worker_pool_is_bounded() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let candidates = (0..20)
        .map(|i| Candidate {
            manager: Manager::System,
            env_path: PathBuf::from(format!("env{i}")),
            python_path: PathBuf::from(format!("python{i}")),
            probe_mode: ProbeMode::Interpreter,
        })
        .collect();
    let a = Arc::clone(&active);
    let p = Arc::clone(&peak);
    let found = app::probe_candidates_with(candidates, "x", move |_, _| {
        let n = a.fetch_add(1, Ordering::SeqCst) + 1;
        p.fetch_max(n, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(2));
        a.fetch_sub(1, Ordering::SeqCst);
        Some(ProbeResult {
            import_match: true,
            import_name: Some("x".into()),
            ..Default::default()
        })
    });
    assert_eq!(found.len(), 20);
    let limit = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 8);
    assert!(peak.load(Ordering::SeqCst) <= limit);
}

#[test]
fn completed_no_match_is_not_a_result() {
    let candidate = Candidate {
        manager: Manager::System,
        env_path: PathBuf::from("env"),
        python_path: PathBuf::from("python"),
        probe_mode: ProbeMode::Interpreter,
    };
    let found = app::probe_candidates_with(vec![candidate], "missing", |_, _| {
        Some(ProbeResult::default())
    });
    assert!(found.is_empty());
}

#[test]
fn static_project_candidate_is_never_sent_to_interpreter_probe() {
    let candidate = Candidate {
        manager: Manager::Uv,
        env_path: PathBuf::from(r"C:\project\.venv"),
        python_path: PathBuf::from(r"C:\project\.venv\Scripts\python.exe"),
        probe_mode: ProbeMode::StaticMetadata,
    };
    let found = app::probe_candidates_with(vec![candidate], "scipy", |_, _| {
        panic!("inactive project interpreter must not execute")
    });
    assert!(found.is_empty());
}
