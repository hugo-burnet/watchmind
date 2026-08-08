use std::{path::PathBuf, process::Command};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn import(ratings: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_watchmind-cli"))
        .args([
            "import-csv",
            fixture(ratings).to_str().unwrap(),
            "--catalog",
            fixture("fixtures/synthetic/catalog.json").to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

#[test]
fn command_prints_a_stable_summary() {
    let first = import("fixtures/synthetic/ratings.csv");
    let second = import("fixtures/synthetic/ratings.csv");

    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        "import ok: catalog=8 ratings=4 completed=3 dropped=1 rewatches=2\n"
    );
}

#[test]
fn command_returns_actionable_errors_for_invalid_fixtures() {
    let cases = [
        (
            "fixtures/invalid/duplicate-rating.csv",
            "duplicate work_id 1",
        ),
        (
            "fixtures/invalid/rating-out-of-range.csv",
            "field rating, value \"10.5\"",
        ),
        (
            "fixtures/invalid/inconsistent-drop.csv",
            "drop position must be lower than a non-zero total",
        ),
        (
            "fixtures/invalid/unknown-work.csv",
            "identifier is absent from catalog.json",
        ),
    ];

    for (fixture, expected) in cases {
        let output = import(fixture);
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("ratings.csv at line"),
            "missing location in {stderr:?}"
        );
        assert!(
            stderr.contains(expected),
            "missing {expected:?} in {stderr:?}"
        );
    }
}
