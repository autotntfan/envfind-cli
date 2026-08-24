use envfind::discovery::DiscoveryProvider;
use envfind::discovery::conda::CondaProvider;
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

fn clear_env(key: &'static str) -> EnvGuard {
    let previous = env::var_os(key);
    unsafe { env::remove_var(key) };
    EnvGuard { key, previous }
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "envfind-conda-discovery-{}-{}",
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

fn conda_fixture(root: &Path) {
    fs::create_dir_all(root.join("conda-meta")).unwrap();
    fixture_python(&root.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    }));
}

#[test]
fn discovers_registered_and_configured_prefixes_without_recursive_scan() {
    let root = temp_dir();
    let home = root.join("home");
    let envs_dir = root.join("envs");
    let custom_dir = root.join("custom-envs");
    let named = envs_dir.join("named");
    let nested = named.join("envs/nested");
    let registered = root.join("registered");
    let custom = custom_dir.join("custom");
    for path in [&named, &nested, &registered, &custom] {
        conda_fixture(path);
    }

    fs::create_dir_all(home.join(".conda")).unwrap();
    fs::write(
        home.join(".conda/environments.txt"),
        format!(
            "# comment\n{}\n{}\n",
            registered.display(),
            root.join("missing").display()
        ),
    )
    .unwrap();
    fs::write(
        home.join(".condarc"),
        format!("envs_dirs:\n  - {}\n", custom_dir.display()),
    )
    .unwrap();

    let _home = set_env("USERPROFILE", &home);
    let _conda_prefix = clear_env("CONDA_PREFIX");
    let _conda_exe = clear_env("CONDA_EXE");
    let _envs_path = set_env("CONDA_ENVS_PATH", &envs_dir);
    let found = CondaProvider.discover();

    assert!(
        found
            .iter()
            .any(|c| c.manager == Manager::Conda && c.env_path == named)
    );
    assert!(found.iter().any(|c| c.env_path == registered));
    assert!(found.iter().any(|c| c.env_path == custom));
    assert!(!found.iter().any(|c| c.env_path == nested));
    let _ = fs::remove_dir_all(root);
}
