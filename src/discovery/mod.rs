use std::collections::HashMap;
use std::env;
use std::fs;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use std::path::Prefix;
use std::path::{Component, Path, PathBuf};

use crate::model::{Candidate, Manager, ProbeMode};

pub mod active;
pub mod conda;
pub mod path;
pub mod pipenv;
pub mod poetry;
pub mod pyenv;
pub mod uv;
pub mod windows_registry;

const MAX_DISCOVERY_ENTRIES: usize = 4096;

pub trait DiscoveryProvider: Send + Sync {
    fn discover(&self) -> Vec<Candidate>;
}

pub fn default_providers() -> Vec<Box<dyn DiscoveryProvider>> {
    vec![
        Box::new(active::ActiveProvider),
        Box::new(windows_registry::WindowsRegistryProvider),
        Box::new(path::PathProvider),
        Box::new(conda::CondaProvider),
        Box::new(uv::UvProvider),
        Box::new(pyenv::PyenvProvider),
        Box::new(poetry::PoetryProvider),
        Box::new(pipenv::PipenvProvider),
    ]
}

pub fn discover_all(providers: &[Box<dyn DiscoveryProvider>]) -> Vec<Candidate> {
    let mut by_python: HashMap<String, Candidate> = HashMap::new();
    for provider in providers {
        for mut candidate in provider.discover() {
            let Some(path) = normalize_windows_path(&candidate.python_path) else {
                continue;
            };
            candidate.python_path = path;
            let Some(env_path) = normalize_windows_path(&candidate.env_path) else {
                continue;
            };
            candidate.env_path = env_path;
            let key = path_key(&candidate.python_path);
            match by_python.get(&key) {
                Some(existing) if existing.manager.priority() <= candidate.manager.priority() => {}
                _ => {
                    by_python.insert(key, candidate);
                }
            }
        }
    }
    let mut candidates: Vec<_> = by_python.into_values().collect();
    candidates.sort_by(|a, b| {
        a.manager
            .priority()
            .cmp(&b.manager.priority())
            .then_with(|| path_key(&a.env_path).cmp(&path_key(&b.env_path)))
            .then_with(|| path_key(&a.python_path).cmp(&path_key(&b.python_path)))
    });
    candidates
}

pub fn normalize_windows_path(path: &Path) -> Option<PathBuf> {
    trusted_absolute_path(path)?;
    let absolute = path.to_path_buf();
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Some(normalized)
}

pub(crate) fn trusted_absolute_path(path: &Path) -> Option<&Path> {
    if !path.is_absolute() {
        return None;
    }
    #[cfg(windows)]
    if let Some(Component::Prefix(prefix)) = path.components().next()
        && !matches!(prefix.kind(), Prefix::Disk(..))
    {
        return None;
    }
    if has_reparse_ancestor(path) {
        return None;
    }
    Some(path)
}

fn has_reparse_ancestor(path: &Path) -> bool {
    let mut current = path;
    loop {
        match fs::symlink_metadata(current) {
            Ok(metadata) if is_reparse_point(&metadata) => return true,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return true,
        }
        let Some(parent) = current.parent() else {
            return false;
        };
        if parent == current {
            return false;
        }
        current = parent;
    }
}

pub(crate) fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}
pub(crate) fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_file() && !is_reparse_point(&m))
        .unwrap_or(false)
}
pub(crate) fn is_regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_dir() && !is_reparse_point(&m))
        .unwrap_or(false)
}
#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
pub(crate) fn python_file(path: &Path) -> bool {
    if !is_regular_file(path) {
        return false;
    }
    #[cfg(windows)]
    {
        use std::io::Read;
        let mut magic = [0u8; 2];
        return fs::File::open(path)
            .and_then(|mut f| f.read_exact(&mut magic))
            .is_ok()
            && magic == *b"MZ";
    }
    #[cfg(not(windows))]
    {
        true
    }
}
pub(crate) fn candidate(
    manager: Manager,
    env_path: PathBuf,
    python_path: PathBuf,
) -> Option<Candidate> {
    trusted_absolute_path(&env_path)?;
    trusted_absolute_path(&python_path)?;
    python_file(&python_path).then_some(Candidate {
        manager,
        env_path,
        python_path,
        probe_mode: ProbeMode::Interpreter,
    })
}
pub(crate) fn immediate_children(root: &Path) -> Vec<PathBuf> {
    if trusted_absolute_path(root).is_none() {
        return Vec::new();
    }
    fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .take(MAX_DISCOVERY_ENTRIES)
        .filter_map(|e| is_regular_directory(&e.path()).then_some(e.path()))
        .collect()
}
pub(crate) fn env_path() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|path| trusted_absolute_path(path).is_some())
}
pub(crate) fn venv_python(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        root.join("Scripts").join("python.exe")
    }
    #[cfg(not(windows))]
    {
        let p = root.join("bin").join("python");
        if p.exists() {
            p
        } else {
            root.join("bin").join("python3")
        }
    }
}
pub(crate) fn conda_candidate(root: PathBuf, manager: Manager) -> Option<Candidate> {
    trusted_absolute_path(&root)?;
    if !is_regular_directory(&root.join("conda-meta")) {
        return None;
    }
    candidate(
        manager,
        root.clone(),
        root.join(if cfg!(windows) {
            "python.exe"
        } else {
            "bin/python"
        }),
    )
}
pub(crate) fn venv_candidate(root: PathBuf, manager: Manager) -> Option<Candidate> {
    trusted_absolute_path(&root)?;
    if is_regular_directory(&root) && is_regular_file(&root.join("pyvenv.cfg")) {
        candidate(manager, root.clone(), venv_python(&root))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_duplicate_case_and_parent_segments() {
        let p = normalize_windows_path(&std::env::temp_dir().join("foo/../Bar")).unwrap();
        assert!(path_key(&p).ends_with("\\bar") || path_key(&p).ends_with("/bar"));
    }
}
