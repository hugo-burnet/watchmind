use std::{fs::File, path::PathBuf};

use watchmind_recommendation::{BaselineKind, EvaluationError, OfflineDataset, evaluate_baselines};

fn fixture(path: &str) -> File {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    File::open(root.join(path)).unwrap()
}

fn synthetic_dataset() -> OfflineDataset {
    OfflineDataset::import(
        fixture("fixtures/synthetic/ratings.csv"),
        fixture("fixtures/synthetic/catalog.json"),
    )
    .unwrap()
}

#[test]
fn evaluates_all_baselines_with_stable_target_order() {
    let report = evaluate_baselines(&synthetic_dataset()).unwrap();

    assert_eq!(report.cases(), 2);
    assert_eq!(report.baselines().len(), 3);
    assert_eq!(report.baselines()[0].name(), BaselineKind::Random);
    assert_eq!(
        report.baselines()[1].name(),
        BaselineKind::AnilistGlobalScore
    );
    assert_eq!(report.baselines()[2].name(), BaselineKind::TagOverlap);
    let expected_ranks = [[4, 5], [1, 1], [4, 4]];
    for (baseline, expected) in report.baselines().iter().zip(expected_ranks) {
        let targets = baseline
            .target_ranks()
            .iter()
            .map(|target| target.work_id().get())
            .collect::<Vec<_>>();
        assert_eq!(targets, [1, 2]);
        let ranks = baseline
            .target_ranks()
            .iter()
            .map(|target| target.rank())
            .collect::<Vec<_>>();
        assert_eq!(ranks, expected);
        assert!(baseline.metrics().median_rank() >= 1.0);
        assert!((baseline.metrics().recall_at_10() - 1.0).abs() < f64::EPSILON);
        assert!((baseline.metrics().recall_at_20() - 1.0).abs() < f64::EPSILON);
        assert!(baseline.metrics().mean_reciprocal_rank() > 0.0);
    }
}

#[test]
fn text_and_json_reports_are_byte_for_byte_deterministic() {
    let dataset = synthetic_dataset();
    let first = evaluate_baselines(&dataset).unwrap();
    let second = evaluate_baselines(&dataset).unwrap();

    assert_eq!(first.to_string(), second.to_string());
    assert_eq!(first.to_json().unwrap(), second.to_json().unwrap());
}

/// Le seuil de pertinence suit la distribution de l'utilisateur. Un noteur qui
/// ne dépasse jamais 6 doit rester évaluable sur ses propres favoris, sans quoi
/// le harness refuserait son historique alors que le reste du moteur raisonne
/// déjà en écart à sa moyenne.
#[test]
fn evaluates_a_harsh_rater_on_their_own_distribution() {
    let ratings = "work_id,rating,status,drop_position,total_episodes,rewatches\n\
                   3,6.0,completed,,,0\n\
                   4,4.0,dropped,3,24,0\n";
    let dataset = OfflineDataset::import(
        ratings.as_bytes(),
        fixture("fixtures/synthetic/catalog.json"),
    )
    .unwrap();

    let report = evaluate_baselines(&dataset).unwrap();
    assert_eq!(report.cases(), 1);
    assert_eq!(report.baselines().len(), 3);
}

#[test]
fn refuses_a_dataset_without_any_rating() {
    let ratings = "work_id,rating,status,drop_position,total_episodes,rewatches\n";
    let dataset = OfflineDataset::import(
        ratings.as_bytes(),
        fixture("fixtures/synthetic/catalog.json"),
    )
    .unwrap();

    assert_eq!(
        evaluate_baselines(&dataset).unwrap_err(),
        EvaluationError::NoRelevantRatings
    );
}
