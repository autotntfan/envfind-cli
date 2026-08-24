use envfind::discovery::{DiscoveryProvider, discover_all};
use envfind::model::{Candidate, Manager, ProbeMode};
struct Provider(Vec<Candidate>);
impl DiscoveryProvider for Provider {
    fn discover(&self) -> Vec<Candidate> {
        self.0.clone()
    }
}
#[test]
fn higher_priority_manager_replaces_case_insensitive_duplicate() {
    let p = std::env::temp_dir().join("fixture-python");
    let providers: Vec<Box<dyn DiscoveryProvider>> = vec![
        Box::new(Provider(vec![Candidate {
            manager: Manager::System,
            env_path: std::env::temp_dir().join("system"),
            python_path: p.clone(),
            probe_mode: ProbeMode::Interpreter,
        }])),
        Box::new(Provider(vec![Candidate {
            manager: Manager::Active,
            env_path: std::env::temp_dir().join("active"),
            python_path: std::env::temp_dir().join("FIXTURE-PYTHON"),
            probe_mode: ProbeMode::Interpreter,
        }])),
    ];
    let found = discover_all(&providers);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].manager, Manager::Active);
}
