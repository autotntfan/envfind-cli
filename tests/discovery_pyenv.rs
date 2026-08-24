use envfind::discovery::DiscoveryProvider;
use envfind::discovery::pyenv::PyenvProvider;
use envfind::model::Manager;
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
fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "envfind-pyenv-discovery-{}-{}",
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
fn scans_pyenv_versions_one_level() {
    let root = temp_dir();
    let versions = root.join("versions");
    let direct = versions.join("3.12.0");
    let nested = versions.join("group/3.13.0");
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
    let _pyenv_root = set_env("PYENV_ROOT", &root);
    let _profile = set_env("USERPROFILE", &root.join("profile"));
    let found = PyenvProvider.discover();
    assert!(
        found
            .iter()
            .any(|c| c.manager == Manager::Pyenv && c.env_path == direct)
    );
    assert!(!found.iter().any(|c| c.env_path == nested));
    let _ = fs::remove_dir_all(root);
}
