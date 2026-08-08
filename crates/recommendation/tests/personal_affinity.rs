use watchmind_recommendation::{
    AffinityConfig, AffinityError, OfflineDataset, RatingSignalKind, WorkId, calculate_affinities,
};

const AFFINITY_CONFIG: &str = include_str!("../../../fixtures/config/affinity-v1.json");

fn dataset(rows: &str, works: &[(u32, u32)]) -> OfflineDataset {
    let csv = format!("work_id,rating,status,drop_position,total_episodes,rewatches\n{rows}");
    let catalog = works
        .iter()
        .map(|(id, runtime)| {
            format!(
                r#"{{"id":{id},"title":"Work {id}","global_score":8.0,"runtime_minutes":{runtime},"tags":[]}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let catalog = format!("[{catalog}]");

    OfflineDataset::import(csv.as_bytes(), catalog.as_bytes()).unwrap()
}

fn affinity_value(dataset: &OfflineDataset, work_id: u32) -> f64 {
    calculate_affinities(dataset, &AffinityConfig::default())
        .unwrap()
        .affinity_for(WorkId::new(work_id).unwrap())
        .unwrap()
        .value()
}

#[test]
fn configuration_fixture_matches_the_v1_defaults() {
    let fixture: AffinityConfig = serde_json::from_str(AFFINITY_CONFIG).unwrap();
    assert_eq!(fixture, AffinityConfig::default());
    assert_eq!(
        serde_json::to_value(fixture).unwrap(),
        serde_json::to_value(AffinityConfig::default()).unwrap()
    );
}

#[test]
fn centers_ratings_on_the_personal_mean_and_exposes_an_exact_breakdown() {
    let dataset = dataset(
        "1,9,completed,,,0\n2,5,completed,,,0\n",
        &[(1, 300), (2, 300)],
    );
    let report = calculate_affinities(&dataset, &AffinityConfig::default()).unwrap();

    assert!((report.personal_mean().get() - 7.0).abs() < f64::EPSILON);
    let positive = report.affinity_for(WorkId::new(1).unwrap()).unwrap();
    let negative = report.affinity_for(WorkId::new(2).unwrap()).unwrap();
    assert!((positive.rating_signal() - 1.0).abs() < f64::EPSILON);
    assert!((negative.rating_signal() + 1.0).abs() < f64::EPSILON);
    for affinity in report.affinities() {
        let sum = affinity.rating_signal() + affinity.rewatch_bonus() + affinity.drop_penalty();
        assert!((affinity.value() - sum).abs() < f64::EPSILON);
    }
}

#[test]
fn treats_good_but_not_for_me_as_a_distinct_soft_negative_signal() {
    let dataset = dataset(
        "1,10,completed,,,0\n2,9,completed,,,0\n3,7,completed,,,0\n",
        &[(1, 300), (2, 300), (3, 300)],
    );
    let report = calculate_affinities(&dataset, &AffinityConfig::default()).unwrap();
    let affinity = report.affinity_for(WorkId::new(3).unwrap()).unwrap();
    let raw_centered = (7.0 - report.personal_mean().get()) / 2.0;

    assert_eq!(
        affinity.rating_signal_kind(),
        RatingSignalKind::GoodButNotForMe
    );
    assert!((affinity.rating_signal() - raw_centered * 0.5).abs() < f64::EPSILON);
    assert!(affinity.value() < 0.0);
}

#[test]
fn rewatch_count_never_reduces_affinity_and_has_diminishing_returns() {
    let mut previous_value = f64::NEG_INFINITY;
    let mut previous_increment = f64::INFINITY;

    for rewatches in 0..=100 {
        let dataset = dataset(&format!("1,8,completed,,,{rewatches}\n"), &[(1, 300)]);
        let value = affinity_value(&dataset, 1);
        assert!(value >= previous_value);

        if rewatches > 0 {
            let increment = value - previous_value;
            assert!(increment > 0.0);
            assert!(increment <= previous_increment);
            previous_increment = increment;
        }
        previous_value = value;
    }
}

#[test]
fn a_longer_rewatch_is_a_stronger_but_bounded_signal() {
    let dataset = dataset(
        "1,8,completed,,,1\n2,8,completed,,,1\n3,8,completed,,,1\n",
        &[(1, 30), (2, 300), (3, 30_000)],
    );
    let report = calculate_affinities(&dataset, &AffinityConfig::default()).unwrap();
    let short = report.affinity_for(WorkId::new(1).unwrap()).unwrap();
    let reference = report.affinity_for(WorkId::new(2).unwrap()).unwrap();
    let long = report.affinity_for(WorkId::new(3).unwrap()).unwrap();

    assert!(short.rewatch_bonus() < reference.rewatch_bonus());
    assert!(reference.rewatch_bonus() < long.rewatch_bonus());
    assert!((short.rewatch_bonus() / reference.rewatch_bonus() - 0.5).abs() < 1.0e-12);
    assert!((long.rewatch_bonus() / reference.rewatch_bonus() - 2.0).abs() < 1.0e-12);
}

#[test]
fn an_earlier_drop_always_penalizes_more_than_a_later_drop() {
    let mut previous = f64::NEG_INFINITY;
    for position in 0..24 {
        let dataset = dataset(&format!("1,6,dropped,{position},24,0\n"), &[(1, 576)]);
        let value = affinity_value(&dataset, 1);
        assert!(value > previous);
        previous = value;
    }
}

#[test]
fn refuses_an_empty_history() {
    let dataset = dataset("", &[(1, 300)]);
    assert_eq!(
        calculate_affinities(&dataset, &AffinityConfig::default()).unwrap_err(),
        AffinityError::NoRatings
    );
}

#[test]
fn refuses_a_non_finite_result_from_extreme_but_finite_parameters() {
    let config: AffinityConfig =
        serde_json::from_str(r#"{"rewatch_weight":1.7976931348623157e308}"#).unwrap();
    let dataset = dataset("1,8,completed,,,2\n", &[(1, 300)]);

    assert_eq!(
        calculate_affinities(&dataset, &config).unwrap_err(),
        AffinityError::InvalidComputedAffinity {
            work_id: WorkId::new(1).unwrap()
        }
    );
}
