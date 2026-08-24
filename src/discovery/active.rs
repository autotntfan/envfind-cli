use super::{DiscoveryProvider, conda_candidate, venv_candidate};
use crate::model::Manager;
use std::env;
use std::path::PathBuf;

pub struct ActiveProvider;
impl DiscoveryProvider for ActiveProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut out = Vec::new();
        if let Some(root) = env::var_os("VIRTUAL_ENV").map(PathBuf::from) {
            if let Some(c) = venv_candidate(root, Manager::Active) {
                out.push(c);
            }
        }
        if let Some(root) = env::var_os("CONDA_PREFIX").map(PathBuf::from) {
            if let Some(c) = conda_candidate(root, Manager::Active) {
                out.push(c);
            }
        }
        out
    }
}
