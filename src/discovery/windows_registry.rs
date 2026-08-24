use super::{DiscoveryProvider, candidate};
use crate::model::{Candidate, Manager};
use std::path::PathBuf;

pub(crate) trait RegistryReader {
    fn executable_paths(&self) -> Vec<PathBuf>;
}

struct SystemRegistryReader;

impl RegistryReader for SystemRegistryReader {
    fn executable_paths(&self) -> Vec<PathBuf> {
        #[cfg(windows)]
        {
            let Some(reg) = system_reg_executable() else {
                return Vec::new();
            };
            let mut paths = Vec::new();
            for hive in [
                r"HKCU\Software\Python\PythonCore",
                r"HKLM\Software\Python\PythonCore",
                r"HKLM\Software\WOW6432Node\Python\PythonCore",
            ] {
                let Ok(output) = std::process::Command::new(&reg)
                    .args(["query", hive, "/s"])
                    .output()
                else {
                    continue;
                };
                if output.status.success() {
                    paths.extend(parse_registry_output(&String::from_utf8_lossy(
                        &output.stdout,
                    )));
                }
            }
            paths
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }
}

fn parse_registry_output(stdout: &str) -> Vec<PathBuf> {
    let mut current_key = None;
    let mut paths = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("HKEY_") {
            current_key = Some(trimmed);
            continue;
        }
        let Some(key_path) = current_key else {
            continue;
        };
        let Some((name, value)) = trimmed.split_once("REG_SZ") else {
            continue;
        };
        let name = name.trim();
        if !is_pep514_install_key(key_path)
            || !(name.eq_ignore_ascii_case("(Default)")
                || name.eq_ignore_ascii_case("ExecutablePath"))
        {
            continue;
        }
        let value = PathBuf::from(value.trim());
        paths.push(if name.eq_ignore_ascii_case("(Default)") {
            value.join("python.exe")
        } else {
            value
        });
    }
    paths
}

fn is_pep514_install_key(key: &str) -> bool {
    let parts = key.split('\\').collect::<Vec<_>>();
    let Some(index) = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("PythonCore"))
    else {
        return false;
    };
    parts.len() == index + 3
        && !parts[index + 1].is_empty()
        && parts[index + 2].eq_ignore_ascii_case("InstallPath")
}

pub struct WindowsRegistryProvider;
impl DiscoveryProvider for WindowsRegistryProvider {
    fn discover(&self) -> Vec<Candidate> {
        discover_from_reader(&SystemRegistryReader)
    }
}

fn discover_from_reader<R: RegistryReader + ?Sized>(reader: &R) -> Vec<Candidate> {
    reader
        .executable_paths()
        .into_iter()
        .filter_map(|python| {
            let env_path = python.parent().map(PathBuf::from)?;
            candidate(Manager::System, env_path, python)
        })
        .collect()
}

#[cfg(windows)]
fn system_reg_executable() -> Option<PathBuf> {
    let mut buffer = [0u16; 260];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if length == 0 || length as usize >= buffer.len() {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize])).join("reg.exe"))
}

#[cfg(windows)]
unsafe extern "system" {
    fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    struct StaticReader(Vec<PathBuf>);
    impl RegistryReader for StaticReader {
        fn executable_paths(&self) -> Vec<PathBuf> {
            self.0.clone()
        }
    }
    #[test]
    fn registry_discovery_uses_injected_reader() {
        let root = std::env::temp_dir().join(format!(
            "envfind-registry-test-{}-{}",
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
        let bytes = b"MZ\0\0";
        #[cfg(not(windows))]
        let bytes = b"python";
        std::fs::write(&python, bytes).unwrap();
        let found = discover_from_reader(&StaticReader(vec![python.clone()]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manager, Manager::System);
        assert_eq!(found[0].python_path, python);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn registry_parser_rejects_nested_non_pep514_keys() {
        let output = r#"
HKEY_CURRENT_USER\Software\Python\PythonCore\3.12\InstallPath
    (Default)    REG_SZ    C:\Python312
HKEY_CURRENT_USER\Software\Python\PythonCore\3.12\Nested\InstallPath
    (Default)    REG_SZ    C:\Untrusted
"#;
        let paths = parse_registry_output(output);
        assert_eq!(paths, vec![PathBuf::from(r"C:\Python312\python.exe")]);
    }
}
