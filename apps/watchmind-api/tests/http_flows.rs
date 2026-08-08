use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use watchmind_api::{ApiState, CatalogResponse, router, secured_router};
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

    let late_candidate = json!({
        "id": 20, "title": "Naruto", "global_score": 7.9,
        "tags": [{"name": "Crime", "weight": 0.4}, {"name": "Adventure", "weight": 0.8}]
    });
    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/20",
        Some(json!({ "work": late_candidate, "comment": null })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, refreshed) = call(&app, "GET", "/api/recommendations", None).await;
    assert_eq!(refreshed["profile_version"], 3);
    assert!(
        refreshed["recommendations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|recommendation| recommendation["work_id"] == 20)
    );

    let (status, removed) = call(&app, "DELETE", "/api/library/5114", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["profile_version"], 4);
    let (_, library) = call(&app, "GET", "/api/library", None).await;
    assert_eq!(library.as_array().unwrap().len(), 2);
    let (_, historical) = call(&app, "GET", "/api/profile/2/recommendations", None).await;
    assert_eq!(historical["recommendations"][0]["work_id"], 5114);

    let (status, removed) = call(&app, "DELETE", "/api/library/1535", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["profile_version"], 5);
    let (_, profile) = call(&app, "GET", "/api/profile", None).await;
    assert_eq!(profile["profile"]["history_size"], 0);
    let (_, fallback) = call(&app, "GET", "/api/recommendations", None).await;
    assert_eq!(fallback["recommendations"][0]["work_id"], 20);
    assert_eq!(
        fallback["recommendations"][0]["explanation"]["reasons"][0]["source"]["kind"],
        "anilist_prior"
    );
}

#[tokio::test]
async fn secured_router_requires_the_configured_bearer_token() {
    let db = Database::in_memory().await.unwrap();
    let app = secured_router(
        ApiState::with_search(
            db,
            || 0,
            |_, _, _, _| async {
                Ok(CatalogResponse {
                    works: Vec::new(),
                    from_cache: false,
                })
            },
        ),
        Some("secret".to_owned()),
    );
    let unauthorized = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(unauthorized).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let authorized = Request::builder()
        .uri("/api/health")
        .header("authorization", "Bearer secret")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(authorized).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn database_backup_can_be_exported_cleared_and_restored() {
    let db = Database::in_memory().await.unwrap();
    let app = router(ApiState::with_search(
        db,
        || 0,
        |_, _, _, _| async {
            Ok(CatalogResponse {
                works: Vec::new(),
                from_cache: false,
            })
        },
    ));
    let work = json!({
        "id": 1535, "title": "Death Note", "global_score": 8.6,
        "tags": [{"name": "Crime", "weight": 0.9}]
    });
    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/1535",
        Some(json!({ "work": work, "comment": "Repère" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(
        &app,
        "PUT",
        "/api/library/1535/rating",
        Some(json!({ "rating": 9.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, backup) = call(&app, "GET", "/api/database", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(backup["version"], 3);
    assert_eq!(backup["rating_dates"][0], json!([1535, 0]));

    let (status, _) = call(&app, "DELETE", "/api/database", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, empty) = call(&app, "GET", "/api/library", None).await;
    assert!(empty.as_array().unwrap().is_empty());

    let (status, _) = call(&app, "PUT", "/api/database", Some(backup)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, restored) = call(&app, "GET", "/api/library", None).await;
    assert_eq!(restored[0]["work"]["title"], "Death Note");
    assert_eq!(restored[0]["library"]["comment"], "Repère");
}
