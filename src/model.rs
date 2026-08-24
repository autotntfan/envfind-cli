use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Manager {
    Active,
    Conda,
    Uv,
    Pyenv,
    Poetry,
    Pipenv,
    System,
}

impl Manager {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Conda => "conda",
            Self::Uv => "uv",
            Self::Pyenv => "pyenv",
            Self::Poetry => "poetry",
            Self::Pipenv => "pipenv",
            Self::System => "system",
        }
    }
    pub fn priority(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Conda => 1,
            Self::Poetry => 2,
            Self::Pipenv => 3,
            Self::Uv => 4,
            Self::Pyenv => 5,
            Self::System => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub manager: Manager,
    pub env_path: PathBuf,
    pub python_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProbeResult {
    pub import_match: bool,
    pub distribution_match: bool,
    pub import_name: Option<String>,
    pub distribution_name: Option<String>,
    pub providers: Vec<String>,
    pub top_level_imports: Vec<String>,
}
