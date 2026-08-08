use std::{path::PathBuf, process::Command};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn recommend(extra_arguments: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_watchmind-cli"));
    command.args([
        "recommend",
        fixture("fixtures/synthetic/ratings.csv").to_str().unwrap(),
        "--catalog",
        fixture("fixtures/synthetic/catalog.json").to_str().unwrap(),
    ]);
    command.args(extra_arguments).output().unwrap()
}

#[test]
fn command_reports_filtering_and_explains_ranked_candidates_deterministically() {
    let first = recommend(&[]);
    let second = recommend(&[]);

    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let report = String::from_utf8(first.stdout).unwrap();
    assert!(report.starts_with("candidates: catalog=8 accepted=3 seen=4"));
    assert!(report.contains("Raisons :"));
    assert!(report.contains("Risques :"));
    assert!(!report.contains("Lanterns at Noon"));
}

#[test]
fn command_exposes_the_same_filter_report_and_scores_as_json() {
    let output = recommend(&["--json"]);
    assert!(output.status.success());
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(document["candidate_report"]["seen"], 4);
    assert_eq!(document["candidate_report"]["availability"], 1);
    assert_eq!(document["recommendations"].as_array().unwrap().len(), 3);
}
