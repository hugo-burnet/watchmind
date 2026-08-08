use std::{fs::File, path::PathBuf};

use watchmind_recommendation::{ImportError, ImportSummary, OfflineDataset};

fn fixture(path: &str) -> File {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    File::open(root.join(path)).unwrap()
}

fn import(ratings: &str) -> Result<OfflineDataset, ImportError> {
    OfflineDataset::import(fixture(ratings), fixture("fixtures/synthetic/catalog.json"))
}

#[test]
fn synthetic_dataset_covers_liked_neutral_dropped_and_rewatched_works() {
    let dataset = import("fixtures/synthetic/ratings.csv").unwrap();

    assert_eq!(
        dataset.summary(),
        ImportSummary {
            catalog: 8,
            ratings: 4,
            completed: 3,
            dropped: 1,
            rewatches: 2,
        }
    );
    assert!(
        dataset
            .ratings()
            .iter()
            .any(|item| item.rating().get() >= 8.0)
    );
    assert!(
        dataset
            .ratings()
            .iter()
            .any(|item| (item.rating().get() - 6.0).abs() < f64::EPSILON)
    );
}

#[test]
fn duplicate_rating_reports_both_lines() {
    let error = import("fixtures/invalid/duplicate-rating.csv").unwrap_err();
    assert!(matches!(
        error,
        ImportError::DuplicateRating {
            first_line: 2,
            duplicate_line: 3,
            ..
        }
    ));
}

#[test]
fn out_of_range_rating_reports_the_field_and_value() {
    let error = import("fixtures/invalid/rating-out-of-range.csv").unwrap_err();
    assert!(matches!(
        error,
        ImportError::InvalidField {
            line: 2,
            field: "rating",
            ref value,
            ..
        } if value == "10.5"
    ));
}

#[test]
fn inconsistent_drop_reports_both_progress_values() {
    let error = import("fixtures/invalid/inconsistent-drop.csv").unwrap_err();
    assert!(matches!(
        error,
        ImportError::InvalidField {
            line: 2,
            field: "drop_position,total_episodes",
            ref value,
            ..
        } if value == "24,24"
    ));
}

#[test]
fn unknown_identifier_reports_its_line() {
    let error = import("fixtures/invalid/unknown-work.csv").unwrap_err();
    assert!(matches!(error, ImportError::UnknownWork { line: 2, .. }));
}
