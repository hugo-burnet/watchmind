use serde_json::json;
use watchmind_infrastructure::Database;
use watchmind_recommendation::{NormalizedWork, Rating, TagWeight, Weight, WorkId};

/// Une contribution dont la représentation courte demande dix-sept chiffres
/// significatifs. Sans la feature `float_roundtrip` de `serde_json`, la
/// relecture décale d'un ULP.
const SEVENTEEN_DIGIT_CONTRIBUTION: f64 = -0.027_428_481_568_539_994;

#[tokio::test]
async fn score_snapshots_survive_a_round_trip_without_losing_a_bit() {
    let db = Database::in_memory().await.unwrap();
    let work = NormalizedWork::new(
        WorkId::new(20).unwrap(),
        "Naruto",
        Some(Rating::new(7.9).unwrap()),
        vec![TagWeight::new("Crime", Weight::new(0.4).unwrap()).unwrap()],
    )
    .unwrap();
    db.works().upsert(&work).await.unwrap();

    let profile = json!({ "confidence": 0.310_344_827_586_206_9 });
    let scores = vec![json!({
        "work_id": 20,
        "title": "Naruto",
        "score": {
            "total": SEVENTEEN_DIGIT_CONTRIBUTION,
            "contributions": [{
                "source": { "kind": "penalty" },
                "value": SEVENTEEN_DIGIT_CONTRIBUTION,
                "detail": "Risque appris pour le tag Crime"
            }]
        }
    })];

    let version = db
        .snapshots()
        .create(1_700_000_000, &profile, &scores)
        .await
        .unwrap();
    let stored = db.snapshots().scores(version).await.unwrap();

    assert_eq!(stored, scores);
    let value = stored[0]["score"]["contributions"][0]["value"]
        .as_f64()
        .unwrap();
    assert!(
        value.to_bits() == SEVENTEEN_DIGIT_CONTRIBUTION.to_bits(),
        "relu {value:?} au lieu de {SEVENTEEN_DIGIT_CONTRIBUTION:?}"
    );

    let latest = db.snapshots().latest_profile().await.unwrap().unwrap();
    assert_eq!(latest.profile, profile);
}
