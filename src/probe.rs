use std::io::Read;
#[cfg(windows)]
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::model::ProbeResult;

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_PROBE_STDOUT: usize = 1024 * 1024;
const PROBE_SCRIPT: &str = r#"import importlib.metadata as metadata
import importlib.util
import json
import re
import sys

query = sys.argv[1]
def canonicalize(name):
    return re.sub(r"[-_.]+", "-", name).lower()
def valid_import_name(name):
    return bool(name) and all(part.isidentifier() for part in name.split("."))

import_match = False
if valid_import_name(query):
    try:
        import_match = importlib.util.find_spec(query) is not None
    except Exception:
        import_match = False

wanted = canonicalize(query)
distribution_match = False
matched_distribution = None
try:
    for dist in metadata.distributions():
        name = dist.metadata.get("Name")
        if name and canonicalize(name) == wanted:
            distribution_match = True
            matched_distribution = name
            break
except Exception:
    pass

providers = []
top_level_imports = []
try:
    mapping = metadata.packages_distributions()
    root = query.split(".", 1)[0]
    providers = list(mapping.get(root, []))
    if distribution_match:
        for package, names in mapping.items():
            if any(canonicalize(name) == wanted for name in names):
                top_level_imports.append(package)
except Exception:
    pass

result = {"import_match": import_match, "distribution_match": distribution_match,
          "import_name": query if import_match else None,
          "distribution_name": matched_distribution, "providers": providers,
          "top_level_imports": top_level_imports}
print("ENVFIND_RESULT=" + json.dumps(result, separators=(",", ":")))
"#;

pub(crate) fn probe_arguments(query: &str) -> [&str; 4] {
    ["-I", "-c", PROBE_SCRIPT, query]
}

pub fn probe(python: &Path, query: &str, timeout: Duration) -> Option<ProbeResult> {
    let cwd = neutral_probe_current_dir()?;
    let mut command = Command::new(python);
    command
        .args(probe_arguments(query))
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        if let Some(v) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", v);
        }
        if let Some(v) = std::env::var_os("WINDIR") {
            command.env("WINDIR", v);
        }
    }
    #[cfg(windows)]
    let job = Job::new().ok()?;
    let mut child = command.spawn().ok()?;
    #[cfg(windows)]
    if job.assign(child.as_raw_handle()).is_err() {
        job.terminate();
        terminate(&mut child);
        return None;
    }
    #[cfg(windows)]
    if resume_suspended_process(child.id()).is_err() {
        job.terminate();
        terminate(&mut child);
        return None;
    }
    let stdout = child.stdout.take()?;
    let stdout_reader = thread::spawn(move || read_capped(stdout));
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let _ = stdout_reader.join();
                    return None;
                }
                let output = stdout_reader.join().ok()??;
                return parse_result(output);
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) | Err(_) => {
                #[cfg(windows)]
                job.terminate();
                terminate(&mut child);
                let _ = stdout_reader.join();
                return None;
            }
        }
    }
}

fn read_capped(mut reader: impl Read) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer).ok()?;
        if count == 0 {
            return Some(output);
        }
        if output.len().saturating_add(count) > MAX_PROBE_STDOUT {
            return None;
        }
        output.extend_from_slice(&buffer[..count]);
    }
}

fn parse_result(output: Vec<u8>) -> Option<ProbeResult> {
    let stdout = String::from_utf8_lossy(&output);
    let line = stdout
        .lines()
        .find_map(|line| line.strip_prefix("ENVFIND_RESULT="))?;
    Some(ProbeResult {
        import_match: json_bool(line, "import_match")?,
        distribution_match: json_bool(line, "distribution_match")?,
        import_name: json_string(line, "import_name"),
        distribution_name: json_string(line, "distribution_name"),
        providers: json_array(line, "providers"),
        top_level_imports: json_array(line, "top_level_imports"),
    })
}

fn json_bool(input: &str, key: &str) -> Option<bool> {
    let value = json_value(input, key)?;
    match value.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}
fn json_string(input: &str, key: &str) -> Option<String> {
    let value = json_value(input, key)?;
    if value == "null" {
        return None;
    }
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .map(unescape)
}
fn json_array(input: &str, key: &str) -> Vec<String> {
    let Some(value) = json_value(input, key) else {
        return Vec::new();
    };
    let Some(body) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return Vec::new();
    };
    body.split(',')
        .filter_map(|part| {
            part.trim()
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .map(unescape)
        })
        .collect()
}
fn json_value(input: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":");
    let start = input.find(&marker)? + marker.len();
    let rest = input[start..].trim_start();
    if rest.starts_with('"') {
        let mut escaped = false;
        for (i, c) in rest.char_indices().skip(1) {
            if c == '"' && !escaped {
                return Some(rest[..=i].to_owned());
            }
            escaped = c == '\\' && !escaped;
            if c != '\\' {
                escaped = false;
            }
        }
        None
    } else if rest.starts_with('[') {
        let end = rest.find(']')?;
        Some(rest[..=end].to_owned())
    } else {
        let end = rest.find(',').unwrap_or(rest.len());
        Some(rest[..end].trim_end_matches('}').trim().to_owned())
    }
}
fn unescape(value: &str) -> String {
    value.replace("\\\"", "\"").replace("\\\\", "\\")
}
fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn resume_suspended_process(process_id: u32) -> std::io::Result<()> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    let mut entry = ThreadEntry::default();
    entry.size = std::mem::size_of::<ThreadEntry>() as u32;
    let mut found = None;
    let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    while has_entry {
        if entry.owner_process_id == process_id {
            found = Some(entry.thread_id);
            break;
        }
        has_entry = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    unsafe {
        CloseHandle(snapshot);
    }
    let Some(thread_id) = found else {
        return Err(std::io::Error::other("probe thread not found"));
    };
    let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    if thread.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let result = unsafe { ResumeThread(thread) };
    unsafe {
        CloseHandle(thread);
    }
    if result == u32::MAX {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct ThreadEntry {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_SUSPENDED: u32 = 0x0000_0004;
#[cfg(windows)]
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
#[cfg(windows)]
const THREAD_SUSPEND_RESUME: u32 = 0x0002;
#[cfg(windows)]
const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;

#[cfg(windows)]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut std::ffi::c_void;
    fn Thread32First(snapshot: *mut std::ffi::c_void, entry: *mut ThreadEntry) -> i32;
    fn Thread32Next(snapshot: *mut std::ffi::c_void, entry: *mut ThreadEntry) -> i32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> *mut std::ffi::c_void;
    fn ResumeThread(thread: *mut std::ffi::c_void) -> u32;
}

#[cfg(windows)]
struct Job(*mut std::ffi::c_void);

#[cfg(windows)]
impl Job {
    fn new() -> std::io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let mut limits = JobLimits::default();
        limits.basic.flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                handle,
                9,
                (&mut limits as *mut JobLimits).cast(),
                std::mem::size_of::<JobLimits>() as u32,
            )
        };
        if ok == 0 {
            unsafe {
                CloseHandle(handle);
            }
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(handle))
    }
    fn assign(&self, process: *mut std::ffi::c_void) -> std::io::Result<()> {
        if unsafe { AssignProcessToJobObject(self.0, process) } == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    fn terminate(&self) {
        unsafe {
            let _ = TerminateJobObject(self.0, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
#[repr(C)]
struct JobBasic {
    _user_time: i64,
    _kernel_time: i64,
    flags: u32,
    _flags_padding: u32,
    _rest: [u8; 40],
}
#[cfg(windows)]
impl Default for JobBasic {
    fn default() -> Self {
        Self {
            _user_time: 0,
            _kernel_time: 0,
            flags: 0,
            _flags_padding: 0,
            _rest: [0; 40],
        }
    }
}
#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct IoCounters {
    _values: [u64; 6],
}
#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct JobLimits {
    basic: JobBasic,
    io: IoCounters,
    _memory: [usize; 4],
}
#[cfg(windows)]
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

#[cfg(windows)]
unsafe extern "system" {
    fn AssignProcessToJobObject(job: *mut std::ffi::c_void, process: *mut std::ffi::c_void) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    fn CreateJobObjectW(attrs: *const std::ffi::c_void, name: *const u16) -> *mut std::ffi::c_void;
    fn SetInformationJobObject(
        job: *mut std::ffi::c_void,
        class: u32,
        info: *mut std::ffi::c_void,
        length: u32,
    ) -> i32;
    fn TerminateJobObject(job: *mut std::ffi::c_void, code: u32) -> i32;
}

#[cfg(windows)]
fn neutral_probe_current_dir() -> Option<PathBuf> {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("WINDIR").map(PathBuf::from))
}
#[cfg(not(windows))]
fn neutral_probe_current_dir() -> Option<PathBuf> {
    Some(PathBuf::from("/"))
}

pub fn render_match(result: &ProbeResult) -> Option<String> {
    if !result.import_match && !result.distribution_match {
        return None;
    }
    match (result.import_match, result.distribution_match) {
        (true, true) => Some(format!(
            "import+package: {}",
            result.import_name.as_deref().unwrap_or("query")
        )),
        (true, false) => Some(match result.providers.first() {
            Some(dist) => format!(
                "import: {} <- {dist}",
                result.import_name.as_deref().unwrap_or("query")
            ),
            None => format!(
                "import: {}",
                result.import_name.as_deref().unwrap_or("query")
            ),
        }),
        (false, true) => Some(match result.top_level_imports.first() {
            Some(import) => format!(
                "package: {} -> {import}",
                result.distribution_name.as_deref().unwrap_or("query")
            ),
            None => format!(
                "package: {}",
                result.distribution_name.as_deref().unwrap_or("query")
            ),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn json_parser_reads_protocol() {
        let input = r#"{"import_match":true,"distribution_match":false,"import_name":"x","distribution_name":null,"providers":["dist"],"top_level_imports":[]}"#;
        assert_eq!(json_bool(input, "import_match"), Some(true));
        assert_eq!(json_string(input, "import_name").as_deref(), Some("x"));
        assert_eq!(json_array(input, "providers"), vec!["dist"]);
    }

    #[test]
    fn probe_uses_direct_interpreter_arguments() {
        let args = probe_arguments("query with spaces");
        assert_eq!(args[0], "-I");
        assert_eq!(args[1], "-c");
        assert_eq!(args[3], "query with spaces");
        assert_eq!(Path::new("python.exe").file_name().unwrap(), "python.exe");
    }
}
