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

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn library_flow_versions_profiles_and_preserves_old_explanations() {
    let db = Database::in_memory().await.unwrap();
    let works =
        AniListNormalizer::normalize(include_str!("../../../fixtures/anilist/search-anime.json"))
            .unwrap();
    let app = router(ApiState::with_search(
        db,
        || 1_700_000_000,
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

    let (status, search) = call(&app, "GET", "/api/anime/search?q=death%20note", None).await;
    assert_eq!(status, StatusCode::OK);
    let anilist_work = search["works"][0].clone();
    assert_eq!(anilist_work["id"], 1535);

    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/1535",
        Some(json!({
            "work": anilist_work, "comment": "Un duel psychologique marquant"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, library) = call(&app, "GET", "/api/library", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(library.as_array().unwrap().len(), 1);
    assert_eq!(library[0]["work"]["title"], "Death Note");
    assert_eq!(
        library[0]["library"]["comment"],
        "Un duel psychologique marquant"
    );

    let candidate = json!({
        "id": 5114, "title": "Fullmetal Alchemist: Brotherhood", "global_score": 9.1,
        "tags": [{"name": "Crime", "weight": 0.7}, {"name": "Adventure", "weight": 0.9}]
    });
    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/5114",
        Some(json!({
            "work": candidate, "comment": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, first) = call(
        &app,
        "PUT",
        "/api/library/1535/rating",
        Some(json!({
            "rating": 9.0, "aspects": [{"axis": "story", "credit": 0.9}]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["profile_version"], 1);

    for event in [
        json!({"kind": "dropped", "work_id": 1535, "progress": {"position": 3, "total": 37}}),
        json!({"kind": "rewatched", "work_id": 1535}),
    ] {
        let (status, _) = call(&app, "POST", "/api/library/1535/events", Some(event)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    let (status, complete) = call(&app, "GET", "/api/works/1535", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(complete["rating"]["rating"], 9.0);
    assert_eq!(complete["rating"]["aspects"][0]["axis"], "story");
    assert_eq!(complete["events"].as_array().unwrap().len(), 2);
    assert_eq!(
        complete["library"]["comment"],
        "Un duel psychologique marquant"
    );

    let (_, before) = call(&app, "GET", "/api/profile/1/recommendations", None).await;
    let old_explanation = before["recommendations"][0]["explanation"].clone();
    let (status, second) = call(
        &app,
        "PUT",
        "/api/library/1535/rating",
        Some(json!({
            "rating": 4.0, "aspects": [{"axis": "visual_direction", "credit": 0.8}]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["profile_version"], 2);

    let (status, profiles) = call(&app, "GET", "/api/profiles", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profiles.as_array().unwrap().len(), 2);
    assert_eq!(profiles[0]["version"], 2);

    let (status, _) = call(
        &app,
        "POST",
        "/api/recommendations/5114/feedback",
        Some(json!({ "helpful": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, current) = call(&app, "GET", "/api/recommendations", None).await;
    assert_eq!(current["profile_version"], 2);
    let (_, after) = call(&app, "GET", "/api/profile/1/recommendations", None).await;
    assert_eq!(after["recommendations"][0]["explanation"], old_explanation);
}
