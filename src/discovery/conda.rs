use super::{
    DiscoveryProvider, conda_candidate, env_path, immediate_children, trusted_absolute_path,
};
use crate::model::Manager;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CondaProvider;
impl DiscoveryProvider for CondaProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut direct_prefixes = Vec::new();
        let mut base_roots = Vec::new();
        if let Some(p) = env::var_os("CONDA_PREFIX") {
            direct_prefixes.push(PathBuf::from(p));
        }
        if let Some(v) = env::var_os("CONDA_ENVS_PATH") {
            for p in env::split_paths(&v) {
                direct_prefixes.extend(prefixes_from_envs_dir(&p));
            }
        }
        if let Some(home) = env_path() {
            let file = home.join(".conda/environments.txt");
            if trusted_absolute_path(&file).is_some()
                && let Ok(text) = fs::read_to_string(file)
            {
                direct_prefixes.extend(text.lines().filter_map(|l| {
                    let p = PathBuf::from(l.trim());
                    (!l.trim().is_empty() && !l.trim_start().starts_with('#')).then_some(p)
                }));
            }
        }
        if let Some(exe) = env::var_os("CONDA_EXE") {
            if let Some(root) = PathBuf::from(exe).parent().and_then(Path::parent) {
                base_roots.push(root.to_path_buf());
            }
        }
        if let Some(home) = env_path() {
            for name in ["miniconda3", "anaconda3", "mambaforge", "miniforge3"] {
                base_roots.push(home.join(name));
            }
            for config in [home.join(".condarc"), home.join(".config/conda/.condarc")] {
                if trusted_absolute_path(&config).is_some()
                    && let Ok(text) = fs::read_to_string(config)
                {
                    for root in parse_envs_dirs(&text) {
                        direct_prefixes.extend(prefixes_from_envs_dir(&root));
                    }
                }
            }
        }
        let mut out = Vec::new();
        for prefix in direct_prefixes {
            if let Some(c) = conda_candidate(prefix, Manager::Conda) {
                out.push(c);
            }
        }
        for root in base_roots {
            if let Some(c) = conda_candidate(root.clone(), Manager::Conda) {
                out.push(c);
            }
            for child in immediate_children(&root.join("envs")) {
                if let Some(c) = conda_candidate(child, Manager::Conda) {
                    out.push(c);
                }
            }
        }
        out
    }
}
fn prefixes_from_envs_dir(root: &Path) -> Vec<PathBuf> {
    immediate_children(root)
}
fn parse_envs_dirs(text: &str) -> Vec<PathBuf> {
    text.lines()
        .skip_while(|l| !l.trim().starts_with("envs_dirs:"))
        .skip(1)
        .take_while(|l| l.trim_start().starts_with('-'))
        .filter_map(|l| {
            l.split_once('-')
                .map(|(_, v)| PathBuf::from(v.trim().trim_matches('"')))
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn conda_candidate_requires_conda_metadata() {
        let root = std::env::temp_dir().join(format!(
            "envfind-conda-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let python = root.join(if cfg!(windows) {
            "python.exe"
        } else {
            "bin/python"
        });
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        #[cfg(windows)]
        let fixture = b"MZ\0\0";
        #[cfg(not(windows))]
        let fixture = b"fixture";
        std::fs::write(&python, fixture).unwrap();
        assert!(conda_candidate(root.clone(), Manager::Conda).is_none());
        std::fs::create_dir_all(root.join("conda-meta")).unwrap();
        assert!(conda_candidate(root.clone(), Manager::Conda).is_some());
        let _ = std::fs::remove_dir_all(root);
    }
}
