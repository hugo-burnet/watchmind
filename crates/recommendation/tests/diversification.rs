use std::collections::{BTreeMap, HashSet};

use watchmind_recommendation::{
    CandidateRequest, DiversificationConfig, ExplorationSignal, NormalizedWork, OfflineDataset,
    Rating, RatingRecord, RecommendationEngine, RecommendationKind, TagWeight, TasteProfileConfig,
    WatchEvent, Weight, WorkId, build_taste_profile,
};

const DIVERSIFICATION_CONFIG: &str =
    include_str!("../../../fixtures/config/diversification-v1.json");

fn work(id: u32, score: f64, tag: &str) -> NormalizedWork {
    NormalizedWork::new(
        WorkId::new(id).unwrap(),
        format!("Work {id}"),
        Some(Rating::new(score).unwrap()),
        vec![TagWeight::new(tag, Weight::new(1.0).unwrap()).unwrap()],
    )
    .unwrap()
}

fn candidate(id: u32, score: f64, tag: &str, franchise: &str, studio: &str) -> NormalizedWork {
    work(id, score, tag)
        .with_franchise(franchise)
        .unwrap()
        .with_studios(vec![studio.to_owned()])
        .unwrap()
}

fn sparse_dataset(mut candidates: Vec<NormalizedWork>) -> OfflineDataset {
    let history = vec![
        work(1, 8.0, "Drama"),
        work(2, 8.0, "Mystery"),
        work(3, 6.0, "Comedy"),
    ];
    candidates.extend(history.clone());
    let ratings = vec![
        RatingRecord::new(WorkId::new(1).unwrap(), Rating::new(10.0).unwrap(), vec![]).unwrap(),
        RatingRecord::new(WorkId::new(2).unwrap(), Rating::new(9.0).unwrap(), vec![]).unwrap(),
        RatingRecord::new(WorkId::new(3).unwrap(), Rating::new(1.0).unwrap(), vec![]).unwrap(),
    ];
    let events = (1..=3)
        .map(|id| WatchEvent::completed(WorkId::new(id).unwrap()))
        .collect();
    OfflineDataset::from_parts(candidates, ratings, events).unwrap()
}

fn recommend(
    dataset: &OfflineDataset,
    config: &DiversificationConfig,
) -> watchmind_recommendation::RecommendationList {
    let profile = build_taste_profile(dataset, &TasteProfileConfig::default()).unwrap();
    let engine = RecommendationEngine::default();
    let candidates = engine.generate_candidates(dataset, &CandidateRequest::default());
    engine.recommend(&profile, &candidates, config).unwrap()
}

#[test]
fn configuration_fixture_is_the_documented_v1_default() {
    let fixture: DiversificationConfig = serde_json::from_str(DIVERSIFICATION_CONFIG).unwrap();
    assert_eq!(fixture, DiversificationConfig::default());
}

#[test]
fn defaults_return_eight_safe_choices_and_two_explained_bets_with_all_caps() {
    let tags = ["Drama", "Mystery", "Adventure", "Romance"];
    let candidates = (0..12)
        .map(|index| {
            candidate(
                100 + index,
                9.0 - f64::from(index) / 10.0,
                tags[index as usize % tags.len()],
                &format!("Franchise {index}"),
                &format!("Studio {}", index / 2),
            )
        })
        .collect();
    let dataset = sparse_dataset(candidates);
    let first = recommend(&dataset, &DiversificationConfig::default());
    let second = recommend(&dataset, &DiversificationConfig::default());

    assert_eq!(first, second);
    assert_eq!(first.safe_count(), 8);
    assert_eq!(first.exploration_count(), 2);
    assert_eq!(first.recommendations().len(), 10);
    assert!(first.recommendations()[..8].iter().all(|recommendation| {
        recommendation.kind() == RecommendationKind::Safe && recommendation.exploration().is_none()
    }));
    assert!(first.recommendations()[8..].iter().all(|recommendation| {
        recommendation.kind() == RecommendationKind::Exploration
            && recommendation
                .exploration()
                .is_some_and(|label| !label.text().is_empty())
    }));

    let selected = first
        .recommendations()
        .iter()
        .map(|recommendation| recommendation.scored().work_id())
        .collect::<HashSet<_>>();
    let mut franchises = BTreeMap::<String, usize>::new();
    let mut studios = BTreeMap::<String, usize>::new();
    let mut tags = BTreeMap::<String, usize>::new();
    for work in dataset
        .catalog()
        .iter()
        .filter(|work| selected.contains(&work.id()))
    {
        *franchises
            .entry(work.franchise().unwrap().to_owned())
            .or_default() += 1;
        *studios.entry(work.studios()[0].clone()).or_default() += 1;
        *tags.entry(work.tags()[0].name().to_owned()).or_default() += 1;
    }
    assert!(franchises.values().all(|count| *count <= 1));
    assert!(studios.values().all(|count| *count <= 2));
    assert!(tags.values().all(|count| *count <= 3));
}

#[test]
fn mmr_prefers_a_distinct_work_over_a_near_clone() {
    let dataset = sparse_dataset(vec![
        work(100, 9.0, "Drama"),
        work(101, 8.9, "Drama"),
        work(102, 8.9, "Mystery"),
    ]);
    let config: DiversificationConfig = serde_json::from_str(
        r#"{
            "safe_count":2,
            "exploration_count":0,
            "mmr_relevance_weight":0.4,
            "max_per_franchise":10,
            "max_per_studio":10,
            "max_per_dominant_tag":10,
            "dominant_tags_per_work":1
        }"#,
    )
    .unwrap();
    let recommendations = recommend(&dataset, &config);
    let ids = recommendations
        .recommendations()
        .iter()
        .map(|recommendation| recommendation.scored().work_id().get())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec![100, 102]);
}

#[test]
fn feasibility_lookahead_keeps_the_requested_size() {
    let dataset = sparse_dataset(vec![
        candidate(100, 9.0, "Drama", "Shared franchise", "Shared studio"),
        candidate(101, 8.0, "Drama", "Shared franchise", "Other studio"),
        candidate(102, 8.0, "Drama", "Other franchise", "Shared studio"),
    ]);
    let config: DiversificationConfig = serde_json::from_str(
        r#"{
            "safe_count":2,
            "exploration_count":0,
            "mmr_relevance_weight":1.0,
            "max_per_franchise":1,
            "max_per_studio":1,
            "max_per_dominant_tag":10,
            "dominant_tags_per_work":1
        }"#,
    )
    .unwrap();
    let recommendations = recommend(&dataset, &config);
    let ids = recommendations
        .recommendations()
        .iter()
        .map(|recommendation| recommendation.scored().work_id().get())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec![101, 102]);
}

#[test]
fn a_bet_can_be_explained_by_disagreement_between_learned_poles() {
    let mut catalog = Vec::new();
    let mut ratings = Vec::new();
    let mut events = Vec::new();
    for id in 1..=30 {
        let (tag, rating) = match id {
            1..=10 => ("Drama", 9.0),
            11..=20 => ("Science Fiction", 9.0),
            _ => ("Comedy", 1.0),
        };
        catalog.push(work(id, 8.0, tag));
        ratings.push(
            RatingRecord::new(
                WorkId::new(id).unwrap(),
                Rating::new(rating).unwrap(),
                vec![],
            )
            .unwrap(),
        );
        events.push(WatchEvent::completed(WorkId::new(id).unwrap()));
    }
    catalog.push(work(100, 8.0, "Drama"));
    let dataset = OfflineDataset::from_parts(catalog, ratings, events).unwrap();
    let config: DiversificationConfig = serde_json::from_str(
        r#"{
            "safe_count":0,
            "exploration_count":1,
            "mmr_relevance_weight":0.75,
            "max_per_franchise":1,
            "max_per_studio":2,
            "max_per_dominant_tag":3,
            "dominant_tags_per_work":2
        }"#,
    )
    .unwrap();
    let recommendations = recommend(&dataset, &config);
    let label = recommendations.recommendations()[0].exploration().unwrap();

    assert_eq!(label.signal(), ExplorationSignal::PoleDisagreement);
    assert!(label.text().contains("pôles de goût divergent"));
}

#[test]
fn rejects_empty_requests_and_zero_caps() {
    for invalid in [
        r#"{"safe_count":0,"exploration_count":0}"#,
        r#"{"max_per_studio":0}"#,
        r#"{"dominant_tags_per_work":0}"#,
    ] {
        assert!(serde_json::from_str::<DiversificationConfig>(invalid).is_err());
    }
}
