use super::{DiscoveryProvider, immediate_children, venv_candidate};
use crate::model::Manager;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct PoetryProvider;
impl DiscoveryProvider for PoetryProvider {
    fn discover(&self) -> Vec<crate::model::Candidate> {
        let mut roots = Vec::new();
        if let Some(root) = env::var_os("POETRY_VIRTUALENVS_PATH") {
            roots.push(PathBuf::from(root));
        }
        if let Some(appdata) = env::var_os("APPDATA") {
            let config = PathBuf::from(&appdata).join("pypoetry/config.toml");
            if let Some(path) = configured_path(&config) {
                roots.push(path);
            }
            roots.push(PathBuf::from(appdata).join("pypoetry/Cache/virtualenvs"));
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            roots.push(PathBuf::from(profile).join("AppData/Local/pypoetry/Cache/virtualenvs"));
        }
        roots
            .into_iter()
            .flat_map(|root| {
                immediate_children(&root)
                    .into_iter()
                    .filter_map(|p| venv_candidate(p, Manager::Poetry))
            })
            .collect()
    }
}
fn configured_path(path: &Path) -> Option<PathBuf> {
    parse_configured_path(&fs::read_to_string(path).ok()?)
}

fn parse_configured_path(text: &str) -> Option<PathBuf> {
    let mut in_virtualenvs = false;
    for line in text.lines() {
        let line = line.split_once('#').map_or(line, |(line, _)| line).trim();
        if line.starts_with('[') {
            in_virtualenvs = line == "[virtualenvs]";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if (in_virtualenvs && key.trim() == "path")
            || (!in_virtualenvs && key.trim() == "virtualenvs.path")
        {
            return Some(PathBuf::from(value.trim().trim_matches(['"', '\''])));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_configured_path;

    #[test]
    fn reads_virtualenvs_section_path() {
        let source = "[virtualenvs]\npath = \"C:/poetry-envs\"\n";
        assert_eq!(
            parse_configured_path(source),
            Some(std::path::PathBuf::from("C:/poetry-envs"))
        );
        assert_eq!(
            parse_configured_path("virtualenvs.path = 'C:/legacy-envs'"),
            Some(std::path::PathBuf::from("C:/legacy-envs"))
        );
    }
}
