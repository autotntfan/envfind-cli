use std::fs;

#[test]
fn release_sources_use_latest_without_manual_version() {
    let installer = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/install.ps1"))
        .expect("install.ps1 should be readable");
    let readme = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("README.md should be readable");

    assert!(installer.contains(r#"$version = "latest""#));
    assert!(installer.contains("releases/latest/download"));
    assert!(readme.contains("releases/latest/download/install.ps1"));
}
