use std::{path::PathBuf, process::Command};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_watchmind-cli"))
        .args(arguments)
        .output()
        .unwrap()
}

fn common(command: &str, extra: &[&str]) -> std::process::Output {
    let ratings = fixture("fixtures/synthetic/ratings.csv");
    let catalog = fixture("fixtures/synthetic/catalog.json");
    let mut arguments = vec![
        command,
        ratings.to_str().unwrap(),
        "--catalog",
        catalog.to_str().unwrap(),
    ];
    arguments.extend_from_slice(extra);
    run(&arguments)
}

#[test]
fn raw_fixtures_cross_every_v1_cli_stage() {
    let imported = common("import-csv", &[]);
    assert!(imported.status.success());
    assert!(
        String::from_utf8(imported.stdout)
            .unwrap()
            .contains("ratings=4")
    );

    let profile = common("build-profile", &["--json"]);
    assert!(profile.status.success());
    let profile: serde_json::Value = serde_json::from_slice(&profile.stdout).unwrap();
    assert_eq!(profile["history_size"], 4);

    let poles = common("show-poles", &["--json"]);
    assert!(poles.status.success());
    assert!(
        !serde_json::from_slice::<serde_json::Value>(&poles.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );

    let recommendations = common("recommend", &["--json"]);
    assert!(recommendations.status.success());
    let recommendations: serde_json::Value =
        serde_json::from_slice(&recommendations.stdout).unwrap();
    let first_id = recommendations["recommendations"][0]["recommendation"]["work_id"]
        .as_u64()
        .unwrap()
        .to_string();

    let ratings = fixture("fixtures/synthetic/ratings.csv");
    let catalog = fixture("fixtures/synthetic/catalog.json");
    let explanation = run(&[
        "explain",
        &first_id,
        ratings.to_str().unwrap(),
        "--catalog",
        catalog.to_str().unwrap(),
        "--json",
    ]);
    assert!(explanation.status.success());
    let explanation: serde_json::Value = serde_json::from_slice(&explanation.stdout).unwrap();
    assert_eq!(
        explanation["work_id"].as_u64().unwrap().to_string(),
        first_id
    );
    assert!(
        !explanation["score"]["contributions"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let evaluation_config = fixture("fixtures/config/evaluation-v1.json");
    let evaluation = common(
        "evaluate",
        &["--config", evaluation_config.to_str().unwrap(), "--json"],
    );
    assert!(evaluation.status.success());
    let evaluation: serde_json::Value = serde_json::from_slice(&evaluation.stdout).unwrap();
    assert_eq!(evaluation["passed"], true);

    let leave_one_out = common(
        "leave-one-out",
        &["--config", evaluation_config.to_str().unwrap(), "--json"],
    );
    assert!(leave_one_out.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&leave_one_out.stdout).unwrap()["target_ranks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let baselines = common("compare-baselines", &["--json"]);
    assert!(baselines.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&baselines.stdout).unwrap()["baselines"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}
