use super::{DiscoveryProvider, immediate_children, venv_candidate};
use crate::model::Manager;
use std::env;
use std::path::PathBuf;

pub struct PipenvProvider;
impl DiscoveryProvider for PipenvProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut roots = Vec::new();
        if let Some(root) = env::var_os("WORKON_HOME") {
            roots.push(PathBuf::from(root));
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            roots.push(PathBuf::from(profile).join(".virtualenvs"));
        }
        roots
            .into_iter()
            .flat_map(|root| {
                immediate_children(&root)
                    .into_iter()
                    .filter_map(|p| venv_candidate(p, Manager::Pipenv))
            })
            .collect()
    }
}
