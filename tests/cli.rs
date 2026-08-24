use envfind::cli::parse_from;
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
