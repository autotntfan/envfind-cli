# envfind

Find managed Windows Python environments that can resolve an import name or installed distribution name.

```powershell
envfind sklearn
envfind scikit-learn
```

Both queries can find the same environment: `sklearn` is an import name; `scikit-learn` is a distribution name. Distribution matching treats `-`, `_`, and `.` as equivalent separators and does not use network access.

Example:

```text
MANAGER  ENV                              PYTHON                                   MATCH
conda    C:\\Users\\me\\miniconda3\\envs\\ml C:\\Users\\me\\miniconda3\\envs\\ml\\python.exe import: sklearn <- scikit-learn
```

## Discovery boundary

envfind does not scan entire drives or recursively search for files named `python.exe`. It discovers active environments, registered/PATH Python installations, Conda-family roots, uv-managed Python roots, pyenv-win versions, Poetry centralized virtualenvs, Pipenv centralized virtualenvs, and the current project's uv-style `.venv` through a bounded parent lookup. Provider directories are enumerated shallowly.

Unrelated project-local `.venv` directories are not globally searched. A `.venv` is included when active through `VIRTUAL_ENV`, explicitly selected by `UV_PROJECT_ENVIRONMENT`, or located directly under the current project/parent path (at most 16 parents) with `pyproject.toml`, `uv.toml`, or `.python-version`. Descendant and sibling projects are not searched. Active/explicit environments use isolated interpreter probing. Inactive marked project environments use direct `site-packages` metadata lookup and never execute their `python.exe`. Broken or timed-out interpreters are skipped.

For each approved candidate, envfind runs its interpreter directly with Python isolated mode (`-I`) and a local standard-library probe. No shell, package manager, telemetry, or network is used during lookup.

## Install on Windows

Download the latest installer to disk, inspect it if needed, then run it:

```powershell
Invoke-WebRequest https://github.com/autotntfan/envfind-cli/releases/latest/download/install.ps1 -OutFile install.ps1
.\install.ps1
```

The installer verifies the downloaded PE file and SHA-256 checksum, installs to `%LOCALAPPDATA%\envfind\bin`, adds that directory to the user PATH, and removes temporary files. It does not require `irm | iex` or `-ExecutionPolicy Bypass`.
