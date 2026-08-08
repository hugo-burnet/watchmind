use std::fmt::Write as _;

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
    let raw_centered = (7.0 - report.personal_mean().get()) / report.rating_scale();

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

/// Un noteur qui plafonne entre 7 et 8 doit produire des signaux comparables à
/// un noteur qui utilise toute la plage. Avec une échelle fixe, le premier
/// produisait des signaux cinq fois plus faibles à goût équivalent.
#[test]
fn adapts_the_rating_scale_to_the_personal_dispersion() {
    let rows = |low: &str, high: &str| {
        let mut csv = String::new();
        for index in 1..=60 {
            let rating = if index % 2 == 0 { high } else { low };
            writeln!(csv, "{index},{rating},completed,,,0").unwrap();
        }
        csv
    };
    let works = (1..=60).map(|id| (id, 300)).collect::<Vec<_>>();

    let compressed = dataset(&rows("7.0", "8.0"), &works);
    let spread = dataset(&rows("5.0", "10.0"), &works);
    let compressed = calculate_affinities(&compressed, &AffinityConfig::default()).unwrap();
    let spread = calculate_affinities(&spread, &AffinityConfig::default()).unwrap();

    assert!(compressed.rating_scale() < spread.rating_scale());
    assert!(compressed.rating_scale() > 0.0);

    let top = |report: &watchmind_recommendation::AffinityReport| {
        report
            .affinity_for(WorkId::new(2).unwrap())
            .unwrap()
            .rating_signal()
    };
    let ratio = top(&spread) / top(&compressed);
    assert!(
        ratio < 2.0,
        "les deux noteurs restent comparables, ratio observé {ratio}"
    );

    let fixed_ratio = ((10.0 - 7.5) / 2.0) / ((8.0 - 7.5) / 2.0);
    assert!(
        fixed_ratio > 4.0,
        "une échelle fixe aurait creusé l'écart, ratio {fixed_ratio}"
    );
}

/// Un abandon sans note est le signal négatif le plus franc de l'historique :
/// il ne doit pas être invisible pour le profil.
#[test]
fn learns_from_drops_that_were_never_rated() {
    use watchmind_recommendation::{
        DropProgress, NormalizedWork, Rating, RatingRecord, WatchEvent,
    };

    let catalog = vec![
        NormalizedWork::new(
            WorkId::new(1).unwrap(),
            "Noté",
            Some(Rating::new(8.0).unwrap()),
            Vec::new(),
        )
        .unwrap(),
        NormalizedWork::new(
            WorkId::new(2).unwrap(),
            "Abandonné sans note",
            Some(Rating::new(8.0).unwrap()),
            Vec::new(),
        )
        .unwrap(),
    ];
    let ratings = vec![
        RatingRecord::new(
            WorkId::new(1).unwrap(),
            Rating::new(8.0).unwrap(),
            Vec::new(),
        )
        .unwrap(),
    ];
    let events = vec![WatchEvent::dropped(
        WorkId::new(2).unwrap(),
        DropProgress::new(1, 24).unwrap(),
    )];
    let dataset = OfflineDataset::from_parts(catalog, ratings, events).unwrap();

    let report = calculate_affinities(&dataset, &AffinityConfig::default()).unwrap();
    let dropped = report.affinity_for(WorkId::new(2).unwrap()).unwrap();

    assert_eq!(dropped.rating_signal_kind(), RatingSignalKind::Unrated);
    assert!((dropped.rating_signal() - 0.0).abs() < f64::EPSILON);
    assert!(dropped.drop_penalty() < 0.0);
    assert!(dropped.value() < 0.0);
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
