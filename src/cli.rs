#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub query: String,
}

pub fn parse_from<I, S>(args: I) -> Result<Cli, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    if args.len() != 2 || args[1].starts_with('-') || args[1].is_empty() {
        return Err("expected exactly one query".into());
    }
    Ok(Cli {
        query: args[1].clone(),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandLine {
    Help,
    Version,
    Run(Cli),
}
pub fn parse_command<I, S>(args: I) -> Result<CommandLine, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Ok(CommandLine::Help);
    }
    if args.len() == 2 && args[1] == "--version" {
        return Ok(CommandLine::Version);
    }
    parse_from(args).map(CommandLine::Run)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_exactly_one_query() {
        assert_eq!(parse_from(["envfind", "sklearn"]).unwrap().query, "sklearn");
    }
    #[test]
    fn rejects_missing_query() {
        assert!(parse_from(["envfind"]).is_err());
    }
    #[test]
    fn rejects_extra_positionals() {
        assert!(parse_from(["envfind", "numpy", "extra"]).is_err());
    }
}
