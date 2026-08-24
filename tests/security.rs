use envfind::discovery::active::ActiveProvider;
use envfind::discovery::conda::CondaProvider;
use envfind::discovery::path::PathProvider;
use envfind::discovery::uv::UvProvider;
use envfind::discovery::{DiscoveryProvider, default_providers, discover_all};
use envfind::model::{Candidate, Manager};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct FixedProvider(Vec<Candidate>);
impl DiscoveryProvider for FixedProvider {
    fn discover(&self) -> Vec<Candidate> {
        self.0.clone()
    }
}

struct EnvCleanup {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}
impl Drop for EnvCleanup {
    fn drop(&mut self) {
        unsafe {
            match self.previous.take() {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }
}
fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
fn set_env(key: &'static str, value: impl AsRef<Path>) -> EnvCleanup {
    let previous = env::var_os(key);
    unsafe {
        env::set_var(key, value.as_ref());
    }
    EnvCleanup { key, previous }
}
fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "envfind-security-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn fixture_python(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    #[cfg(windows)]
    let bytes = b"MZ\0\0";
    #[cfg(not(windows))]
    let bytes = b"python";
    fs::write(path, bytes).unwrap();
}
#[test]
fn arbitrary_tree_has_no_provider_provenance() {
    let root = temp_dir("arbitrary");
    fixture_python(&root.join("random/python.exe"));
    let _path = set_env("PATH", &root);
    let found = discover_all(&[Box::new(PathProvider)]);
    assert!(
        found
            .iter()
            .all(|candidate| !candidate.python_path.starts_with(&root.join("random")))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn relative_candidates_are_not_resolved_from_current_directory() {
    let found = discover_all(&[Box::new(FixedProvider(vec![Candidate {
        manager: Manager::System,
        env_path: PathBuf::from("relative-env"),
        python_path: PathBuf::from("relative-env/python.exe"),
    }]))]);
    assert!(found.is_empty());
}
#[test]
fn discovery_uses_only_explicit_providers() {
    let found = discover_all(&default_providers());
    assert!(
        found
            .iter()
            .all(|c| c.manager != Manager::Active || c.env_path != PathBuf::from("."))
    );
}
#[test]
fn probe_arguments_are_not_shell_commands() {
    let source = include_str!("../src/probe.rs");
    assert!(source.contains("Command::new(python)"));
    assert!(source.contains(".args(probe_arguments(query))"));
    assert!(!source.contains("cmd.exe"));
    assert!(!source.contains("powershell"));
}

#[test]
fn unrelated_project_venv_is_excluded_but_active_venv_is_included() {
    let _lock = env_lock();
    let root = temp_dir("project");
    let venv = root.join(".venv");
    fs::create_dir_all(&venv).unwrap();
    fs::write(venv.join("pyvenv.cfg"), "home = python\n").unwrap();
    fixture_python(&venv.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }));
    let _virtual_env = set_env("VIRTUAL_ENV", &root.join("does-not-exist"));
    let _path = set_env("PATH", &root);
    assert!(
        discover_all(&[Box::new(CondaProvider), Box::new(UvProvider)])
            .iter()
            .all(|c| c.python_path
                != venv.join(if cfg!(windows) {
                    "Scripts/python.exe"
                } else {
                    "bin/python"
                }))
    );
    drop(_virtual_env);
    let _active = set_env("VIRTUAL_ENV", &venv);
    let found = ActiveProvider.discover();
    assert!(found.iter().any(|c| c.manager == Manager::Active
        && c.python_path
            == venv.join(if cfg!(windows) {
                "Scripts/python.exe"
            } else {
                "bin/python"
            })));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn provider_roots_do_not_recurse_beyond_immediate_children() {
    let _lock = env_lock();
    let root = temp_dir("uv");
    fixture_python(&root.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    fixture_python(&root.join("deep/env/python.exe"));
    let _uv = set_env("UV_PYTHON_INSTALL_DIR", &root);
    let uv_found = UvProvider.discover();
    assert!(uv_found.is_empty());

    let conda_root = temp_dir("conda");
    let nested = conda_root.join("deep/env");
    fs::create_dir_all(nested.join("conda-meta")).unwrap();
    fixture_python(&nested.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    let _conda = set_env("CONDA_ENVS_PATH", &conda_root);
    let conda_found = CondaProvider.discover();
    assert!(
        conda_found
            .iter()
            .all(|candidate| !candidate.env_path.starts_with(&conda_root)),
        "unexpected nested conda candidates: {conda_found:?}"
    );

    let registered = temp_dir("registered-conda");
    fs::create_dir_all(registered.join("conda-meta")).unwrap();
    fixture_python(&registered.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    let nested_registered = registered.join("envs/nested");
    fs::create_dir_all(nested_registered.join("conda-meta")).unwrap();
    fixture_python(&nested_registered.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    let parent = registered.parent().unwrap().to_path_buf();
    let _registered = set_env("CONDA_ENVS_PATH", &parent);
    let registered_found = CondaProvider.discover();
    assert!(
        registered_found
            .iter()
            .any(|candidate| candidate.env_path == registered)
    );
    assert!(
        registered_found
            .iter()
            .all(|candidate| candidate.env_path != nested_registered)
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(conda_root);
    let _ = fs::remove_dir_all(parent);
}
