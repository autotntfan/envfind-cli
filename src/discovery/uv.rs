use super::{
    DiscoveryProvider, candidate, immediate_children, is_regular_file, trusted_absolute_path,
    venv_candidate,
};
use crate::model::{Manager, ProbeMode};
use std::env;
use std::path::Path;
use std::path::PathBuf;

const MAX_PROJECT_ANCESTORS: usize = 16;

pub struct UvProvider;
impl DiscoveryProvider for UvProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut found = Vec::new();
        if let Some(root) = env::var_os("UV_PROJECT_ENVIRONMENT").map(PathBuf::from)
            && trusted_absolute_path(&root).is_some()
            && let Some(candidate) = venv_candidate(root, Manager::Uv)
        {
            found.push(candidate);
        }
        if let Some(candidate) = project_candidate() {
            found.push(candidate);
        }
        let mut roots = Vec::new();
        if let Some(root) = env::var_os("UV_PYTHON_INSTALL_DIR") {
            roots.push(PathBuf::from(root));
        }
        if let Some(appdata) = env::var_os("APPDATA") {
            roots.push(PathBuf::from(appdata).join("uv/data/python"));
        }
        found.extend(
            roots
                .into_iter()
                .flat_map(|root| {
                    immediate_children(&root)
                        .into_iter()
                        .filter_map(|p| {
                            candidate(
                                Manager::Uv,
                                p.clone(),
                                p.join(if cfg!(windows) {
                                    "python.exe"
                                } else {
                                    "bin/python"
                                }),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        );
        found
    }
}

fn project_candidate() -> Option<crate::model::Candidate> {
    let mut current = env::current_dir().ok()?;
    trusted_absolute_path(&current)?;
    for _ in 0..=MAX_PROJECT_ANCESTORS {
        if trusted_absolute_path(&current).is_some() && has_project_marker(&current) {
            if let Some(mut candidate) = venv_candidate(current.join(".venv"), Manager::Uv) {
                candidate.probe_mode = ProbeMode::StaticMetadata;
                return Some(candidate);
            }
        }
        let parent = current.parent()?;
        if parent == current {
            return None;
        }
        current = parent.to_path_buf();
    }
    None
}

fn has_project_marker(root: &Path) -> bool {
    ["pyproject.toml", "uv.toml", ".python-version"]
        .into_iter()
        .any(|name| is_regular_file(&root.join(name)))
}
