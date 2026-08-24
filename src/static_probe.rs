use crate::discovery::{is_regular_directory, is_regular_file, trusted_absolute_path};
use crate::model::{Candidate, ProbeMode, ProbeResult};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_METADATA_ENTRIES: usize = 4096;
const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_TOP_LEVEL_NAMES: usize = 4096;

pub fn probe(candidate: &Candidate, query: &str) -> Option<ProbeResult> {
    if candidate.probe_mode != ProbeMode::StaticMetadata {
        return None;
    }
    let wanted = canonicalize(query);
    let import_root = query.split('.').next().filter(|part| valid_name(part));
    let mut result = ProbeResult::default();
    for site in site_packages(&candidate.env_path) {
        let Ok(entries) = fs::read_dir(site) else {
            continue;
        };
        for entry in entries.flatten().take(MAX_METADATA_ENTRIES) {
            let path = entry.path();
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            if import_root.is_some_and(|root| root == name)
                && (is_regular_file(&path) || is_regular_directory(&path))
            {
                result.import_match = true;
            }
            if !is_regular_directory(&path) {
                continue;
            }
            let is_metadata_dir = name.ends_with(".dist-info") || name.ends_with(".egg-info");
            if !is_metadata_dir {
                continue;
            }
            let Some(distribution) = metadata_name(&path.join("METADATA"))
                .or_else(|| metadata_name(&path.join("PKG-INFO")))
            else {
                continue;
            };
            let top_levels = top_level_names(&path);
            if canonicalize(&distribution) == wanted {
                result.distribution_match = true;
                result.distribution_name = Some(distribution.clone());
                let remaining = MAX_TOP_LEVEL_NAMES.saturating_sub(result.top_level_imports.len());
                result
                    .top_level_imports
                    .extend(top_levels.iter().take(remaining).cloned());
            }
            if let Some(root) = import_root
                && top_levels.iter().any(|name| name == root)
            {
                result.import_match = true;
                if !result.providers.contains(&distribution) {
                    result.providers.push(distribution);
                }
            }
        }
    }
    if result.import_match {
        result.import_name = Some(query.to_owned());
    }
    (result.import_match || result.distribution_match).then_some(result)
}

fn canonicalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn site_packages(root: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let site = root.join("Lib/site-packages");
        is_regular_directory(&site)
            .then_some(site)
            .into_iter()
            .collect()
    }
    #[cfg(not(windows))]
    {
        let lib = root.join("lib");
        fs::read_dir(lib)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .take(MAX_METADATA_ENTRIES)
            .map(|entry| entry.path())
            .filter(|path| is_regular_directory(path))
            .map(|path| path.join("site-packages"))
            .filter(|path| is_regular_directory(path))
            .collect()
    }
}

fn metadata_name(path: &Path) -> Option<String> {
    trusted_absolute_path(path)?;
    if !is_regular_file(path) {
        return None;
    }
    let text = read_capped(path)?;
    text.lines()
        .find_map(|line| line.strip_prefix("Name:").map(str::trim))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn top_level_names(path: &Path) -> Vec<String> {
    let Some(text) = read_capped(&path.join("top_level.txt")) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|name| valid_name(name))
        .take(MAX_TOP_LEVEL_NAMES)
        .map(str::to_owned)
        .collect()
}

fn read_capped(path: &Path) -> Option<String> {
    trusted_absolute_path(path)?;
    if !is_regular_file(path) {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take((MAX_METADATA_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_METADATA_BYTES {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Manager, ProbeMode};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "envfind-static-probe-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn site_packages(root: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            root.join("Lib/site-packages")
        }
        #[cfg(not(windows))]
        {
            root.join("lib/python3.12/site-packages")
        }
    }

    #[test]
    fn matches_distribution_without_executing_interpreter() {
        let root = temp_dir();
        let site = site_packages(&root);
        let dist = site.join("scipy-1.0.dist-info");
        fs::create_dir_all(&dist).unwrap();
        fs::write(
            dist.join("METADATA"),
            "Metadata-Version: 2.1\nName: scipy\n",
        )
        .unwrap();
        fs::write(dist.join("top_level.txt"), "scipy\n").unwrap();
        let candidate = Candidate {
            manager: Manager::Uv,
            env_path: root.clone(),
            python_path: root.join("fake-python.exe"),
            probe_mode: ProbeMode::StaticMetadata,
        };

        let result = probe(&candidate, "scipy").unwrap();
        assert!(result.import_match);
        assert!(result.distribution_match);
        assert_eq!(result.distribution_name.as_deref(), Some("scipy"));
        let _ = fs::remove_dir_all(root);
    }
}
