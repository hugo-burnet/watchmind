use std::{fs::File, path::PathBuf};

use watchmind_recommendation::{FullEvaluationConfig, OfflineDataset, evaluate_full};

fn fixture(path: &str) -> File {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    File::open(root.join(path)).unwrap()
}

fn dataset() -> OfflineDataset {
    OfflineDataset::import(
        fixture("fixtures/synthetic/ratings.csv"),
        fixture("fixtures/synthetic/catalog.json"),
    )
    .unwrap()
}

fn config() -> FullEvaluationConfig {
    serde_json::from_reader(fixture("fixtures/config/evaluation-v1.json")).unwrap()
}

#[test]
fn complete_report_covers_leave_one_out_regressions_and_temporal_backtest() {
    let report = evaluate_full(&dataset(), &config()).unwrap();

    assert_eq!(report.engine().target_ranks().len(), 2);
    assert_eq!(report.baselines().baselines().len(), 3);
    assert_eq!(report.regressions().len(), 1);
    assert!(report.temporal_backtest().available());
    assert_eq!(report.temporal_backtest().cases(), 2);
    assert!(report.passed(), "{:?}", report.failures());
    assert!(
        report
            .to_markdown()
            .starts_with("# WatchMind evaluation - PASS")
    );
    assert_eq!(
        report.to_json().unwrap(),
        evaluate_full(&dataset(), &config())
            .unwrap()
            .to_json()
            .unwrap()
    );
}

#[test]
fn configured_regression_and_baseline_thresholds_control_the_gate() {
    let failing: FullEvaluationConfig = serde_json::from_str(
        r#"{
          "thresholds": {"minimum_recall_at_10_delta_vs_tags": 1.0, "minimum_mrr_delta_vs_tags": 1.0},
          "regression_pairs": [{"label":"wrong order","preferred_work_id":8,"other_work_id":5}]
        }"#,
    )
    .unwrap();

    let report = evaluate_full(&dataset(), &failing).unwrap();
    assert!(!report.passed());
    assert_eq!(report.failures().len(), 3);
}
