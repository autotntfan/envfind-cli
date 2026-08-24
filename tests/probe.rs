use envfind::model::ProbeResult;
use envfind::probe::{probe, render_match};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn available_python() -> Option<PathBuf> {
    ["python", "python3"].into_iter().find_map(|name| {
        let output = Command::new(name)
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
    })
}

fn compile_helper(source: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "envfind-probe-helper-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let source_path = root.join("main.rs");
    let binary = root.join(if cfg!(windows) {
        "helper.exe"
    } else {
        "helper"
    });
    fs::write(&source_path, source).unwrap();
    let output = Command::new(option_env!("RUSTC").unwrap_or("rustc"))
        .args(["--edition=2024"])
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "helper compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (root, binary)
}
#[test]
fn renders_import_and_package_matches() {
    let import = ProbeResult {
        import_match: true,
        import_name: Some("sklearn".into()),
        providers: vec!["scikit-learn".into()],
        ..Default::default()
    };
    assert_eq!(
        render_match(&import).as_deref(),
        Some("import: sklearn <- scikit-learn")
    );
    let package = ProbeResult {
        distribution_match: true,
        distribution_name: Some("scikit-learn".into()),
        top_level_imports: vec!["sklearn".into()],
        ..Default::default()
    };
    assert_eq!(
        render_match(&package).as_deref(),
        Some("package: scikit-learn -> sklearn")
    );
}
#[test]
fn no_match_is_omitted() {
    assert!(render_match(&ProbeResult::default()).is_none());
}

#[test]
fn direct_python_probe_reports_import_match_without_network() {
    let python = available_python();
    let Some(python) = python else { return };
    let result = probe(&python, "json", std::time::Duration::from_secs(10))
        .expect("probe should return protocol result");
    assert!(result.import_match);
    let missing = probe(
        &python,
        "envfind_module_that_does_not_exist",
        std::time::Duration::from_secs(10),
    )
    .expect("no-match is still valid probe result");
    assert!(!missing.import_match && !missing.distribution_match);
}

#[test]
fn invalid_import_name_is_a_completed_no_match() {
    let Some(python) = available_python() else {
        return;
    };
    let result = probe(&python, "not-an-import", Duration::from_secs(10))
        .expect("probe protocol should complete");
    assert!(!result.import_match);
}

#[test]
fn abnormal_probe_process_is_excluded() {
    let (root, helper) = compile_helper("fn main() { std::process::exit(7); }\n");
    assert!(probe(&helper, "query", Duration::from_secs(1)).is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn hanging_probe_is_terminated_by_timeout() {
    let (root, helper) =
        compile_helper("fn main() { std::thread::sleep(std::time::Duration::from_secs(10)); }\n");
    let started = Instant::now();
    assert!(probe(&helper, "query", Duration::from_millis(100)).is_none());
    assert!(started.elapsed() < Duration::from_secs(2));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn oversized_probe_output_is_rejected() {
    let (root, helper) =
        compile_helper("fn main() { println!(\"{}\", \"x\".repeat(2_000_000)); }\n");
    let started = Instant::now();
    assert!(probe(&helper, "query", Duration::from_secs(1)).is_none());
    assert!(started.elapsed() < Duration::from_secs(2));
    let _ = fs::remove_dir_all(root);
}
