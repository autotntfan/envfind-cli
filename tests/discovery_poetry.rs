use envfind::discovery::DiscoveryProvider;
use envfind::discovery::poetry::PoetryProvider;
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
        "envfind-poetry-discovery-{}-{}",
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
fn reads_configured_and_explicit_poetry_roots_shallowly() {
    let root = temp_dir();
    let explicit_root = root.join("explicit");
    let configured_root = root.join("configured");
    let direct = explicit_root.join("project-abc");
    let nested = direct.join("nested/ignored");
    let configured = configured_root.join("project-def");
    fixture_venv(&direct);
    fixture_venv(&nested);
    fixture_venv(&configured);
    let appdata = root.join("appdata");
    fs::create_dir_all(appdata.join("pypoetry")).unwrap();
    fs::write(
        appdata.join("pypoetry/config.toml"),
        format!("[virtualenvs]\npath = \"{}\"\n", configured_root.display()),
    )
    .unwrap();

    let _explicit = set_env("POETRY_VIRTUALENVS_PATH", &explicit_root);
    let _appdata = set_env("APPDATA", &appdata);
    let _profile = set_env("USERPROFILE", &root.join("profile"));
    let found = PoetryProvider.discover();
    assert!(
        found
            .iter()
            .any(|c| c.manager == Manager::Poetry && c.env_path == direct)
    );
    assert!(found.iter().any(|c| c.env_path == configured));
    assert!(!found.iter().any(|c| c.env_path == nested));
    let _ = fs::remove_dir_all(root);
}
