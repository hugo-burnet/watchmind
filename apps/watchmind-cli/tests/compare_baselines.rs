use std::{path::PathBuf, process::Command};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn compare(extra_arguments: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_watchmind-cli"));
    command.args([
        "compare-baselines",
        fixture("fixtures/synthetic/ratings.csv").to_str().unwrap(),
        "--catalog",
        fixture("fixtures/synthetic/catalog.json").to_str().unwrap(),
    ]);
    command.args(extra_arguments).output().unwrap()
}

#[test]
fn command_prints_the_same_text_report_twice() {
    let first = compare(&[]);
    let second = compare(&[]);

    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let report = String::from_utf8(first.stdout).unwrap();
    assert!(
        report.starts_with("baseline evaluation: cases=2 relevant_rating>=8.0 random_seed=42\n")
    );
    assert!(report.contains("random"));
    assert!(report.contains("anilist_global_score"));
    assert!(report.contains("tag_overlap"));
}

#[test]
fn command_prints_a_stable_json_report() {
    let first = compare(&["--json"]);
    let second = compare(&["--json"]);

    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let document: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(document["configuration"]["random_seed"], 42);
    assert_eq!(document["cases"], 2);
    assert_eq!(document["baselines"].as_array().unwrap().len(), 3);
}
