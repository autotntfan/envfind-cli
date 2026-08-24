use super::{DiscoveryProvider, candidate, immediate_children};
use crate::model::Manager;
use std::env;
use std::path::PathBuf;

pub struct UvProvider;
impl DiscoveryProvider for UvProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut roots = Vec::new();
        if let Some(root) = env::var_os("UV_PYTHON_INSTALL_DIR") {
            roots.push(PathBuf::from(root));
        }
        if let Some(appdata) = env::var_os("APPDATA") {
            roots.push(PathBuf::from(appdata).join("uv/data/python"));
        }
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
            .collect()
    }
}
