use envfind::discovery::DiscoveryProvider;
use envfind::discovery::pipenv::PipenvProvider;
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
        "envfind-pipenv-discovery-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn fixture_venv(path: &Path) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("pyvenv.cfg"), "home = python\n").unwrap();
    #[cfg(windows)]
    let python = path.join("Scripts/python.exe");
    #[cfg(not(windows))]
    let python = path.join("bin/python");
    fs::create_dir_all(python.parent().unwrap()).unwrap();
    #[cfg(windows)]
    let bytes = b"MZ\0\0";
    #[cfg(not(windows))]
    let bytes = b"python";
    fs::write(python, bytes).unwrap();
}

#[test]
fn reads_workon_home_without_project_recursive_scan() {
    let root = temp_dir();
    let workon = root.join("workon");
    let direct = workon.join("project-abc");
    let nested = workon.join("deep/ignored");
    let unrelated = root.join("project/.venv");
    fixture_venv(&direct);
    fixture_venv(&nested);
    fixture_venv(&unrelated);
    let _workon = set_env("WORKON_HOME", &workon);
    let _profile = set_env("USERPROFILE", &root.join("profile"));
    let found = PipenvProvider.discover();
    assert!(
        found
            .iter()
            .any(|c| c.manager == Manager::Pipenv && c.env_path == direct)
    );
    assert!(!found.iter().any(|c| c.env_path == nested));
    assert!(!found.iter().any(|c| c.env_path == unrelated));
    let _ = fs::remove_dir_all(root);
}
