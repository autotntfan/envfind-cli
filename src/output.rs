use crate::model::{Candidate, ProbeResult};
use crate::probe::render_match;

pub fn render_table(matches: &[(Candidate, ProbeResult)]) -> String {
    let mut rows: Vec<_> = matches
        .iter()
        .filter_map(|(candidate, result)| render_match(result).map(|m| (candidate, m)))
        .collect();
    rows.sort_by(|(a, _), (b, _)| {
        a.manager
            .priority()
            .cmp(&b.manager.priority())
            .then_with(|| {
                a.env_path
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .cmp(&b.env_path.to_string_lossy().to_ascii_lowercase())
            })
            .then_with(|| {
                a.python_path
                    .to_string_lossy()
                    .cmp(&b.python_path.to_string_lossy())
            })
    });
    let headers = ["MANAGER", "ENV", "PYTHON", "MATCH"];
    let values: Vec<[String; 4]> = rows
        .into_iter()
        .map(|(c, m)| {
            [
                c.manager.label().into(),
                c.env_path.display().to_string(),
                c.python_path.display().to_string(),
                m,
            ]
        })
        .collect();
    let widths = (0..4)
        .map(|i| {
            values
                .iter()
                .map(|r| r[i].len())
                .max()
                .unwrap_or(0)
                .max(headers[i].len())
        })
        .collect::<Vec<_>>();
    let mut out = format!(
        "{:<w0$}  {:<w1$}  {:<w2$}  {}\n",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        w0 = widths[0],
        w1 = widths[1],
        w2 = widths[2]
    );
    for row in values {
        out.push_str(&format!(
            "{:<w0$}  {:<w1$}  {:<w2$}  {}\n",
            row[0],
            row[1],
            row[2],
            row[3],
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2]
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Manager, ProbeMode, ProbeResult};
    use std::path::PathBuf;
    #[test]
    fn table_has_required_columns_and_full_paths() {
        let c = Candidate {
            manager: Manager::System,
            env_path: PathBuf::from(r"C:\env"),
            python_path: PathBuf::from(r"C:\env\python.exe"),
            probe_mode: ProbeMode::Interpreter,
        };
        let r = ProbeResult {
            import_match: true,
            import_name: Some("json".into()),
            ..Default::default()
        };
        let table = render_table(&[(c, r)]);
        assert!(
            table.contains("MANAGER")
                && table.contains("ENV")
                && table.contains("PYTHON")
                && table.contains("MATCH")
        );
        assert!(table.contains(r"C:\env\python.exe"));
    }
}
