use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use watchmind_api::{ApiState, CatalogResponse, router};
use watchmind_infrastructure::{AniListNormalizer, Database};

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(
            body.map_or_else(String::new, |value| value.to_string()),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

/// Une œuvre notée **avant** d'avoir été recommandée ne prouve rien : elle
/// aurait été regardée de toute façon. Seule une note postérieure à l'affichage
/// atteste que le moteur a servi à quelque chose.
#[tokio::test]
async fn impact_only_counts_works_rated_after_being_shown() {
    let db = Database::in_memory().await.unwrap();
    let works =
        AniListNormalizer::normalize(include_str!("../../../fixtures/anilist/search-anime.json"))
            .unwrap();
    let app = router(ApiState::with_search(
        db,
        || 1_000,
        move |_, _, _, _| {
            let works = works.clone();
            async move {
                Ok(CatalogResponse {
                    works,
                    from_cache: false,
                })
            }
        },
    ));

    let (status, impact) = call(&app, "GET", "/api/recommendations/impact", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(impact["shown"], 0);
    assert_eq!(impact["watched_after_being_shown"], 0);
    assert_eq!(impact["precision"], Value::Null);

    let candidate = json!({
        "id": 5114, "title": "Fullmetal Alchemist: Brotherhood", "global_score": 9.1,
        "tags": [{"name": "Adventure", "weight": 0.9}]
    });
    for id in [1535_u32, 5114] {
        let work = if id == 5114 {
            candidate.clone()
        } else {
            json!({
                "id": 1535, "title": "Death Note", "global_score": 8.5,
                "tags": [{"name": "Psychological", "weight": 0.9}]
            })
        };
        let (status, _) = call(
            &app,
            "PUT",
            &format!("/api/library/{id}"),
            Some(json!({ "work": work, "comment": null })),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // Note posée avant tout affichage : elle ne doit jamais être créditée.
    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/1535/rating",
        Some(json!({ "rating": 9.0, "aspects": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, shown) = call(&app, "GET", "/api/recommendations", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!shown["recommendations"].as_array().unwrap().is_empty());

    let (_, impact) = call(&app, "GET", "/api/recommendations/impact", None).await;
    assert!(impact["shown"].as_u64().unwrap() > 0);
    assert_eq!(
        impact["watched_after_being_shown"], 0,
        "la note de 1535 precede l'affichage"
    );

    // Note posée après l'affichage : elle compte, et elle dépasse la note
    // mondiale, donc elle compte aussi comme découverte.
    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/5114/rating",
        Some(json!({ "rating": 9.5, "aspects": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, impact) = call(&app, "GET", "/api/recommendations/impact", None).await;
    assert_eq!(impact["watched_after_being_shown"], 1);
    assert_eq!(impact["liked"], 1);
    assert_eq!(impact["precision"], 1.0);
    assert_eq!(impact["above_global_score"], 1);
    assert_eq!(impact["discovery_precision"], 1.0);
}
