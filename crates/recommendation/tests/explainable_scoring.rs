use watchmind_recommendation::{
    NormalizedWork, OfflineDataset, Rating, RatingRecord, RecommendationEngine, ScoringConfig,
    TagWeight, TasteProfileConfig, WatchEvent, Weight, WorkId, build_taste_profile,
};

const SCORING_CONFIG: &str = include_str!("../../../fixtures/config/scoring-v1.json");

fn work(id: u32, title: &str, score: f64, tags: &[(&str, f64)]) -> NormalizedWork {
    NormalizedWork::new(
        WorkId::new(id).unwrap(),
        title,
        Some(Rating::new(score).unwrap()),
        tags.iter()
            .map(|(name, weight)| TagWeight::new(*name, Weight::new(*weight).unwrap()).unwrap())
            .collect(),
    )
    .unwrap()
}

fn profile() -> watchmind_recommendation::TasteProfile {
    let catalog = vec![
        work(1, "Loved drama", 8.0, &[("Drama", 1.0), ("Mystery", 0.8)]),
        work(2, "Loved mystery", 8.0, &[("Mystery", 1.0)]),
        work(3, "Rejected comedy", 6.0, &[("Comedy", 1.0)]),
    ];
    let ratings = vec![
        RatingRecord::new(WorkId::new(1).unwrap(), Rating::new(10.0).unwrap(), vec![]).unwrap(),
        RatingRecord::new(WorkId::new(2).unwrap(), Rating::new(9.0).unwrap(), vec![]).unwrap(),
        RatingRecord::new(WorkId::new(3).unwrap(), Rating::new(1.0).unwrap(), vec![]).unwrap(),
    ];
    let events = (1..=3)
        .map(|id| WatchEvent::completed(WorkId::new(id).unwrap()))
        .collect();
    let dataset = OfflineDataset::from_parts(catalog, ratings, events).unwrap();
    build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap()
}

#[test]
fn configuration_fixture_is_the_documented_v1_default() {
    let fixture: ScoringConfig = serde_json::from_str(SCORING_CONFIG).unwrap();
    assert_eq!(fixture, ScoringConfig::default());
}

#[test]
fn total_is_exactly_the_sum_of_atomic_contributions_for_every_case() {
    let candidates = vec![
        work(10, "Balanced", 8.2, &[("Drama", 0.9), ("Comedy", 0.4)]),
        work(11, "Unknown", 7.1, &[("Adventure", 1.0)]),
        work(12, "Risky", 5.0, &[("Comedy", 1.0)]),
    ];
    let recommendations = RecommendationEngine::default()
        .score_candidates(&profile(), &candidates)
        .unwrap();

    for recommendation in recommendations {
        let sum = recommendation
            .score()
            .contributions()
            .iter()
            .map(|contribution| contribution.value().get())
            .sum::<f64>();
        assert!((recommendation.score().total() - sum).abs() < 1.0e-12);
    }
}

#[test]
fn ranks_deterministically_and_projects_three_reasons_and_two_risks_at_most() {
    let candidates = vec![
        work(
            11,
            "Mixed",
            8.0,
            &[("Drama", 1.0), ("Mystery", 1.0), ("Comedy", 1.0)],
        ),
        work(10, "Aligned", 8.0, &[("Drama", 1.0), ("Mystery", 1.0)]),
    ];
    let engine = RecommendationEngine::default();
    let first = engine.score_candidates(&profile(), &candidates).unwrap();
    let second = engine.score_candidates(&profile(), &candidates).unwrap();

    assert_eq!(first, second);
    assert_eq!(first[0].work_id(), WorkId::new(10).unwrap());
    assert!(first[0].explanation().reasons().len() <= 3);
    assert!(first[1].explanation().risks().len() <= 2);
    assert!(first[1].explanation().to_string().contains("Risques :"));
    assert!(first[1].explanation().to_string().contains("Comedy"));
}

#[test]
fn refuses_a_prior_that_is_not_weak() {
    let error = serde_json::from_str::<ScoringConfig>(r#"{"anilist_prior_weight":0.5}"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("anilist_prior_weight"));
}
