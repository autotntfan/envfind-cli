use envfind::discovery::DiscoveryProvider;
use envfind::discovery::uv::UvProvider;
use envfind::model::{Manager, ProbeMode};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match self.previous.take() {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }
}
fn set_env(key: &'static str, value: impl AsRef<Path>) -> EnvGuard {
    let previous = env::var_os(key);
    unsafe { env::set_var(key, value.as_ref()) };
    EnvGuard { key, previous }
}
fn clear_env(key: &'static str) -> EnvGuard {
    let previous = env::var_os(key);
    unsafe { env::remove_var(key) };
    EnvGuard { key, previous }
}
struct CurrentDirGuard(PathBuf);
impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.0).unwrap();
    }
}
fn set_current_dir(path: &Path) -> CurrentDirGuard {
    let previous = env::current_dir().unwrap();
    env::set_current_dir(path).unwrap();
    CurrentDirGuard(previous)
}
fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "envfind-uv-discovery-{}-{}",
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
fn scans_only_immediate_uv_installations() {
    let root = temp_dir();
    let direct = root.join("cpython-3.12");
    let nested = root.join("deep/env");
    fixture_python(&root.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    fixture_python(&direct.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    fixture_python(&nested.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
    let _uv = set_env("UV_PYTHON_INSTALL_DIR", &root);
    let found = UvProvider.discover();
    assert!(
        found
            .iter()
            .any(|c| c.manager == Manager::Uv && c.env_path == direct)
    );
    assert!(!found.iter().any(|c| c.env_path == root));
    assert!(!found.iter().any(|c| c.env_path == nested));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn finds_nearest_project_venv_but_not_unmarked_venv() {
    let root = temp_dir();
    let project = root.join("project");
    let project_cwd = project.join("src/deep");
    let project_venv = project.join(".venv");
    let descendant_venv = project_cwd.join(".venv");
    let unrelated = root.join("unrelated");
    let unrelated_venv = unrelated.join(".venv");
    fs::create_dir_all(&project_cwd).unwrap();
    fs::write(
        project.join("pyproject.toml"),
        "[project]\nname = 'fixture'\n",
    )
    .unwrap();
    fixture_python(&project_venv.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }));
    fs::write(project_venv.join("pyvenv.cfg"), "home = python\n").unwrap();
    fixture_python(&descendant_venv.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }));
    fs::write(descendant_venv.join("pyvenv.cfg"), "home = python\n").unwrap();
    fixture_python(&unrelated_venv.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }));
    fs::write(unrelated_venv.join("pyvenv.cfg"), "home = python\n").unwrap();
    fs::create_dir_all(&unrelated).unwrap();
    let _uv_root = set_env("UV_PYTHON_INSTALL_DIR", &root.join("missing-uv-root"));
    let _appdata = set_env("APPDATA", &root.join("appdata"));
    let _project_env = clear_env("UV_PROJECT_ENVIRONMENT");
    let _cwd = set_current_dir(&project_cwd);
    let found = UvProvider.discover();
    assert!(found.iter().any(|c| c.manager == Manager::Uv
        && c.env_path == project_venv
        && c.probe_mode == ProbeMode::StaticMetadata));
    assert!(!found.iter().any(|c| c.env_path == descendant_venv));

    env::set_current_dir(&unrelated).unwrap();
    let found_unrelated = UvProvider.discover();
    assert!(!found_unrelated.iter().any(|c| c.env_path == unrelated_venv));
    let explicit_venv = root.join("explicit-venv");
    fs::create_dir_all(&explicit_venv).unwrap();
    fs::write(explicit_venv.join("pyvenv.cfg"), "home = python\n").unwrap();
    fixture_python(&explicit_venv.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }));
    let _explicit = set_env("UV_PROJECT_ENVIRONMENT", &explicit_venv);
    let explicit_found = UvProvider.discover();
    assert!(
        explicit_found
            .iter()
            .any(|c| { c.env_path == explicit_venv && c.probe_mode == ProbeMode::Interpreter })
    );
    drop(_cwd);
    let _ = fs::remove_dir_all(root);
}
