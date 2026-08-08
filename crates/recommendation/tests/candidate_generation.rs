use watchmind_recommendation::{
    CandidateFilter, CandidateRequest, NormalizedWork, OfflineDataset, Rating, RatingRecord,
    RecommendationEngine, ReleaseYear, TagWeight, TasteProfileConfig, WatchEvent, Weight,
    WorkFormat, WorkId, build_taste_profile,
};

const CANDIDATE_CONFIG: &str = include_str!("../../../fixtures/config/candidates-v1.json");

fn work(id: u32, score: f64) -> NormalizedWork {
    NormalizedWork::new(
        WorkId::new(id).unwrap(),
        format!("Work {id}"),
        Some(Rating::new(score).unwrap()),
        vec![TagWeight::new("Drama", Weight::new(1.0).unwrap()).unwrap()],
    )
    .unwrap()
    .with_format(WorkFormat::Tv)
    .with_release_year(ReleaseYear::new(2023).unwrap())
}

fn dataset() -> OfflineDataset {
    let catalog = vec![
        work(1, 9.0),
        work(2, 8.0),
        work(3, 8.0).with_format(WorkFormat::Movie),
        work(4, 8.0).with_release_year(ReleaseYear::new(2010).unwrap()),
        work(5, 5.0),
        work(6, 8.0).with_availability(false),
        work(7, 8.0)
            .with_prerequisites(vec![WorkId::new(99).unwrap()])
            .unwrap(),
        work(8, 8.0),
        work(9, 9.0),
    ];
    let ratings = vec![
        RatingRecord::new(WorkId::new(1).unwrap(), Rating::new(9.0).unwrap(), vec![]).unwrap(),
    ];
    OfflineDataset::from_parts(
        catalog,
        ratings,
        vec![WatchEvent::completed(WorkId::new(1).unwrap())],
    )
    .unwrap()
}

fn strict_request() -> CandidateRequest {
    serde_json::from_str(
        r#"{
            "blacklisted":[2],
            "formats":["tv"],
            "minimum_year":2020,
            "maximum_year":2025,
            "minimum_global_score":7.0,
            "require_available":true,
            "require_prerequisites":true,
            "limit":1
        }"#,
    )
    .unwrap()
}

#[test]
fn configuration_fixture_is_the_documented_v1_default() {
    let fixture: CandidateRequest = serde_json::from_str(CANDIDATE_CONFIG).unwrap();
    assert_eq!(fixture, CandidateRequest::default());
}

#[test]
fn reports_the_first_filter_that_eliminates_each_catalog_work() {
    let candidates =
        RecommendationEngine::default().generate_candidates(&dataset(), &strict_request());
    let report = candidates.report();

    assert_eq!(report.catalog_count(), 9);
    assert_eq!(report.accepted_count(), 1);
    for filter in [
        CandidateFilter::Seen,
        CandidateFilter::Blacklisted,
        CandidateFilter::Format,
        CandidateFilter::ReleaseYear,
        CandidateFilter::GlobalScore,
        CandidateFilter::Availability,
        CandidateFilter::Prerequisites,
        CandidateFilter::Limit,
    ] {
        assert_eq!(report.eliminated_by(filter), 1, "filter {filter:?}");
    }
    assert_eq!(candidates.works()[0].id(), WorkId::new(9).unwrap());
}

#[test]
fn seen_work_and_inaccessible_sequel_never_reach_the_scorer() {
    let dataset = dataset();
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();
    let engine = RecommendationEngine::default();
    let candidates = engine.generate_candidates(&dataset, &strict_request());
    let scored = engine
        .score_candidates(&profile, candidates.works())
        .unwrap();
    let scored_ids = scored
        .iter()
        .map(watchmind_recommendation::ScoredRecommendation::work_id)
        .collect::<Vec<_>>();

    assert!(!scored_ids.contains(&WorkId::new(1).unwrap()));
    assert!(!scored_ids.contains(&WorkId::new(7).unwrap()));
}

#[test]
fn validates_ranges_duplicates_and_nonzero_limit() {
    for invalid in [
        r#"{"limit":0}"#,
        r#"{"blacklisted":[2,2]}"#,
        r#"{"formats":["tv","tv"]}"#,
        r#"{"minimum_year":2025,"maximum_year":2020}"#,
    ] {
        assert!(serde_json::from_str::<CandidateRequest>(invalid).is_err());
    }
}
