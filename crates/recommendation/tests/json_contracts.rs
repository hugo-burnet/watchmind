use serde::{Serialize, de::DeserializeOwned};
use watchmind_recommendation::{NormalizedWork, RatingRecord, RecommendationScore, WatchEvent};

const NORMALIZED_WORK: &str = include_str!("../../../fixtures/domain/normalized-work.json");
const RATING_RECORD: &str = include_str!("../../../fixtures/domain/rating-record.json");
const WATCH_EVENTS: &str = include_str!("../../../fixtures/domain/watch-events.json");
const RECOMMENDATION_SCORE: &str =
    include_str!("../../../fixtures/domain/recommendation-score.json");

fn assert_json_round_trip<T>(fixture: &str)
where
    T: DeserializeOwned + Serialize,
{
    let initial: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let contract: T = serde_json::from_str(fixture).unwrap();
    let serialized = serde_json::to_value(contract).unwrap();
    assert_eq!(serialized, initial);
}

#[test]
fn normalized_work_fixture_round_trips() {
    assert_json_round_trip::<NormalizedWork>(NORMALIZED_WORK);
}

#[test]
fn rating_record_fixture_round_trips() {
    assert_json_round_trip::<RatingRecord>(RATING_RECORD);
}

#[test]
fn watch_event_fixture_round_trips() {
    assert_json_round_trip::<Vec<WatchEvent>>(WATCH_EVENTS);
}

#[test]
fn recommendation_score_fixture_round_trips() {
    assert_json_round_trip::<RecommendationScore>(RECOMMENDATION_SCORE);
}

#[test]
fn json_cannot_bypass_numeric_or_aggregate_invariants() {
    assert!(
        serde_json::from_str::<NormalizedWork>(
            r#"{"id":0,"title":"Invalid","global_score":11,"tags":[]}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<RatingRecord>(
            r#"{
            "work_id":1,
            "rating":8,
            "aspects":[
                {"axis":"story","credit":0.8},
                {"axis":"story","credit":0.4}
            ]
        }"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<WatchEvent>(
            r#"{
            "kind":"dropped",
            "work_id":1,
            "progress":{"position":12,"total":12}
        }"#
        )
        .is_err()
    );
}
