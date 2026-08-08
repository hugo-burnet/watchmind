use std::fmt::Write;

use watchmind_recommendation::{
    AspectCredit, AxisWeightSource, NormalizedWork, OfflineDataset, PersonalAxis, ProfileMode,
    Rating, RatingRecord, TagWeight, TasteProfileConfig, WatchEvent, Weight, WorkId,
    build_taste_profile,
};

const PROFILE_CONFIG: &str = include_str!("../../../fixtures/config/taste-profile-v1.json");

type ImportedRow<'a> = (u32, f64, &'a [(&'a str, f64)]);

fn imported_dataset(rows: &[ImportedRow<'_>]) -> OfflineDataset {
    let mut csv = String::from("work_id,rating,status,drop_position,total_episodes,rewatches\n");
    let mut catalog = Vec::new();
    for (work_id, rating, tags) in rows {
        writeln!(csv, "{work_id},{rating},completed,,,0").unwrap();
        catalog.push(serde_json::json!({
            "id": work_id,
            "title": format!("Work {work_id}"),
            "global_score": 7.5,
            "tags": tags
                .iter()
                .map(|(name, weight)| serde_json::json!({"name": name, "weight": weight}))
                .collect::<Vec<_>>()
        }));
    }
    OfflineDataset::import(
        csv.as_bytes(),
        serde_json::to_vec(&catalog).unwrap().as_slice(),
    )
    .unwrap()
}

fn clustered_dataset() -> OfflineDataset {
    let mut rows = Vec::new();
    for work_id in 1..=8 {
        rows.push((work_id, 10.0, vec![("Mecha", 1.0), ("Space", 0.8)]));
    }
    for work_id in 9..=16 {
        rows.push((work_id, 10.0, vec![("Romance", 1.0), ("Drama", 0.8)]));
    }
    for work_id in 17..=30 {
        rows.push((work_id, 4.0, vec![("Comedy", 1.0)]));
    }
    let borrowed = rows
        .iter()
        .map(|(id, rating, tags)| (*id, *rating, tags.as_slice()))
        .collect::<Vec<_>>();
    imported_dataset(&borrowed)
}

#[test]
fn configuration_fixture_is_the_documented_v1_default() {
    let fixture: TasteProfileConfig = serde_json::from_str(PROFILE_CONFIG).unwrap();
    assert_eq!(fixture, TasteProfileConfig::default());
    assert_eq!(
        serde_json::to_value(fixture).unwrap(),
        serde_json::to_value(TasteProfileConfig::default()).unwrap()
    );
}

#[test]
fn learns_exact_shrunk_tag_affinity_and_volume_breadth_confidence() {
    let dataset = imported_dataset(&[(1, 10.0, &[("Shared", 0.8)]), (2, 6.0, &[("Shared", 0.4)])]);
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();
    let shared = profile.tag_affinity("shared").unwrap();

    // L'étendue sature sur le nombre d'œuvres portant le tag, jamais sur la
    // taille de l'historique : noter d'autres œuvres ne peut pas la réduire.
    let expected_affinity = (0.8 - 0.4) / (0.8 + 0.4 + 2.0);
    let expected_confidence = ((0.8 + 0.4) / (0.8 + 0.4 + 2.0)) * (2.0 / (2.0 + 150.0));
    assert!((shared.value() - expected_affinity).abs() < 1.0e-12);
    assert!((shared.confidence().get() - expected_confidence).abs() < 1.0e-12);
    assert_eq!(shared.observed_works(), 2);
    assert!((shared.evidence_weight() - 1.2).abs() < 1.0e-12);
}

#[test]
fn sparse_history_uses_one_explicit_fallback_pole_and_axis_prior() {
    let dataset = imported_dataset(&[(1, 9.0, &[("Drama", 1.0)]), (2, 5.0, &[("Comedy", 1.0)])]);
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();

    assert_eq!(profile.mode(), ProfileMode::SparseHistory);
    assert_eq!(profile.poles().len(), 1);
    assert_eq!(
        profile.poles()[0].representative_work_ids(),
        [WorkId::new(1).unwrap()]
    );
    assert_eq!(profile.axes().source(), AxisWeightSource::Prior);
    for weight in profile.axes().weights() {
        assert!((weight.weight().get() - 0.2).abs() < f64::EPSILON);
    }
}

#[test]
fn synthetic_profile_recovers_two_expected_poles_deterministically() {
    let dataset = clustered_dataset();
    let first = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();
    let second = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.mode(), ProfileMode::Clustered);
    assert_eq!(first.poles().len(), 2);
    let dominant = first
        .poles()
        .iter()
        .map(|pole| pole.dominant_tags()[0].name())
        .collect::<Vec<_>>();
    assert_eq!(dominant, ["Mecha", "Romance"]);
    assert_eq!(first.poles()[0].member_count(), 8);
    assert_eq!(first.poles()[1].member_count(), 8);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn learns_axis_weights_only_after_enough_annotated_works() {
    let mut catalog = Vec::new();
    let mut ratings = Vec::new();
    let mut events = Vec::new();
    for raw_id in 1..=10 {
        let work_id = WorkId::new(raw_id).unwrap();
        catalog.push(
            NormalizedWork::new(
                work_id,
                format!("Work {raw_id}"),
                None,
                vec![TagWeight::new("Drama", Weight::new(1.0).unwrap()).unwrap()],
            )
            .unwrap(),
        );
        ratings.push(
            RatingRecord::new(
                work_id,
                Rating::new(8.0).unwrap(),
                vec![
                    AspectCredit::new(PersonalAxis::Story, Weight::new(0.8).unwrap()).unwrap(),
                    AspectCredit::new(PersonalAxis::VisualDirection, Weight::new(0.2).unwrap())
                        .unwrap(),
                ],
            )
            .unwrap(),
        );
        events.push(WatchEvent::completed(work_id));
    }
    let dataset = OfflineDataset::from_parts(catalog, ratings, events).unwrap();
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();

    assert_eq!(profile.axes().source(), AxisWeightSource::Learned);
    assert_eq!(profile.axes().observed_works(), 10);
    assert!((profile.axes().weight_for(PersonalAxis::Story).get() - 0.8).abs() < 1.0e-12);
    assert!(
        (profile
            .axes()
            .weight_for(PersonalAxis::VisualDirection)
            .get()
            - 0.2)
            .abs()
            < 1.0e-12
    );
    assert!(
        profile
            .axes()
            .weight_for(PersonalAxis::Characters)
            .get()
            .abs()
            < f64::EPSILON
    );
}

#[test]
fn rejects_invalid_profile_configuration() {
    let error = serde_json::from_str::<TasteProfileConfig>(r#"{"tag_shrinkage":0.0}"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("tag_shrinkage"));
}
