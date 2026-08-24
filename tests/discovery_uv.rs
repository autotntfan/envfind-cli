use envfind::discovery::DiscoveryProvider;
use envfind::discovery::uv::UvProvider;
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
