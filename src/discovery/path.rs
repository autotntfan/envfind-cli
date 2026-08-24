use super::{DiscoveryProvider, candidate};
use crate::model::Manager;
use std::env;

pub struct PathProvider;
impl DiscoveryProvider for PathProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let dirs: Vec<_> = env::var_os("PATH")
            .into_iter()
            .flat_map(|v| env::split_paths(&v).collect::<Vec<_>>())
            .collect();
        dirs.into_iter()
            .filter_map(|dir| {
                let python = dir.join(if cfg!(windows) {
                    "python.exe"
                } else {
                    "python"
                });
                candidate(Manager::System, dir, python)
            })
            .collect()
    }
}
