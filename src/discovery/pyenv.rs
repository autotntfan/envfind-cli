use super::{DiscoveryProvider, candidate, immediate_children};
use crate::model::Manager;
use std::env;
use std::path::PathBuf;

pub struct PyenvProvider;
impl DiscoveryProvider for PyenvProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut roots = Vec::new();
        if let Some(root) = env::var_os("PYENV_ROOT") {
            roots.push(PathBuf::from(root).join("versions"));
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            roots.push(PathBuf::from(profile).join(".pyenv/pyenv-win/versions"));
        }
        roots
            .into_iter()
            .flat_map(|root| {
                immediate_children(&root).into_iter().filter_map(|p| {
                    candidate(
                        Manager::Pyenv,
                        p.clone(),
                        p.join(if cfg!(windows) {
                            "python.exe"
                        } else {
                            "bin/python"
                        }),
                    )
                })
            })
            .collect()
    }
}
