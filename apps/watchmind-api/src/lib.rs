//! API HTTP locale de `WatchMind`.

use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use watchmind_infrastructure::{
    AniListCatalog, Database, LibraryEntry, ProfileSnapshot, StorageError,
};
use watchmind_recommendation::{
    AspectCredit, OfflineDataset, Rating, RatingRecord, RecommendationEngine, TasteProfileConfig,
    WatchEvent, WorkId, build_taste_profile, evaluate_baselines,
};

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
type SearchFuture = Pin<Box<dyn Future<Output = Result<CatalogResponse, String>> + Send>>;
type Search = Arc<dyn Fn(String, u32, u8, u64) -> SearchFuture + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
pub struct CatalogResponse {
    pub works: Vec<watchmind_recommendation::NormalizedWork>,
    pub from_cache: bool,
}

#[derive(Clone)]
pub struct ApiState {
    database: Database,
    search: Search,
    clock: Clock,
}

impl ApiState {
    #[must_use]
    pub fn new(database: Database, catalog: AniListCatalog) -> Self {
        Self::with_clock(database, catalog, || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |time| time.as_secs())
        })
    }

    #[must_use]
    pub fn with_clock(
        database: Database,
        catalog: AniListCatalog,
        clock: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        let catalog = Arc::new(catalog);
        Self::with_search(database, clock, move |query, page, per_page, now| {
            let catalog = Arc::clone(&catalog);
            async move {
                catalog
                    .search(&query, page, per_page, now)
                    .await
                    .map(|result| CatalogResponse {
                        works: result.works().to_vec(),
                        from_cache: result.from_cache(),
                    })
                    .map_err(|error| error.to_string())
            }
        })
    }

    #[must_use]
    pub fn with_search<F, Fut>(
        database: Database,
        clock: impl Fn() -> u64 + Send + Sync + 'static,
        search: F,
    ) -> Self
    where
        F: Fn(String, u32, u8, u64) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CatalogResponse, String>> + Send + 'static,
    {
        Self {
            database,
            search: Arc::new(move |query, page, per_page, now| {
                Box::pin(search(query, page, per_page, now))
            }),
            clock: Arc::new(clock),
        }
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/anime/search", get(search_anime))
        .route("/api/works/{id}", get(read_work))
        .route("/api/library", get(library))
        .route("/api/library/{id}", put(upsert_library))
        .route("/api/library/{id}/rating", put(upsert_rating))
        .route("/api/library/{id}/events", post(append_event))
        .route("/api/recommendations", get(recommendations))
        .route(
            "/api/recommendations/{id}/feedback",
            post(recommendation_feedback),
        )
        .route("/api/profile", get(profile))
        .route("/api/profiles", get(profiles))
        .route(
            "/api/profile/{version}/recommendations",
            get(historical_recommendations),
        )
        .route("/api/evaluation", get(evaluation))
        .with_state(state)
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "one")]
    page: u32,
    #[serde(default = "twenty")]
    per_page: u8,
}
const fn one() -> u32 {
    1
}
const fn twenty() -> u8 {
    20
}

async fn search_anime(
    State(state): State<ApiState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, ApiError> {
    if query.q.trim().is_empty() {
        return Err(ApiError::bad_request("q must not be empty"));
    }
    let result = (state.search)(query.q, query.page, query.per_page, (state.clock)())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!(result)))
}

#[derive(Serialize)]
struct CompleteWork {
    work: watchmind_recommendation::NormalizedWork,
    library: Option<LibraryEntry>,
    rating: Option<RatingRecord>,
    events: Vec<WatchEvent>,
}

async fn library(State(state): State<ApiState>) -> Result<Json<Vec<CompleteWork>>, ApiError> {
    let mut result = Vec::new();
    for entry in state.database.library().all().await? {
        let work = state
            .database
            .works()
            .get(entry.work_id)
            .await?
            .ok_or_else(|| ApiError::internal("library work is missing"))?;
        result.push(CompleteWork {
            work,
            rating: state.database.ratings().get(entry.work_id).await?,
            events: state.database.events().for_work(entry.work_id).await?,
            library: Some(entry),
        });
    }
    Ok(Json(result))
}

async fn read_work(
    State(state): State<ApiState>,
    Path(raw_id): Path<u32>,
) -> Result<Json<CompleteWork>, ApiError> {
    let id = work_id(raw_id)?;
    let work = state
        .database
        .works()
        .get(id)
        .await?
        .ok_or_else(|| ApiError::not_found("work not found"))?;
    Ok(Json(CompleteWork {
        work,
        library: state.database.library().get(id).await?,
        rating: state.database.ratings().get(id).await?,
        events: state.database.events().for_work(id).await?,
    }))
}

#[derive(Deserialize)]
struct LibraryInput {
    work: watchmind_recommendation::NormalizedWork,
    comment: Option<String>,
}

async fn upsert_library(
    State(state): State<ApiState>,
    Path(raw_id): Path<u32>,
    Json(input): Json<LibraryInput>,
) -> Result<StatusCode, ApiError> {
    let id = work_id(raw_id)?;
    if input.work.id() != id {
        return Err(ApiError::bad_request("path and work identifiers differ"));
    }
    state.database.works().upsert(&input.work).await?;
    state
        .database
        .library()
        .upsert(&LibraryEntry {
            work_id: id,
            comment: normalize_comment(input.comment),
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct RatingInput {
    rating: Rating,
    #[serde(default)]
    aspects: Vec<AspectCredit>,
}

async fn upsert_rating(
    State(state): State<ApiState>,
    Path(raw_id): Path<u32>,
    Json(input): Json<RatingInput>,
) -> Result<Json<Value>, ApiError> {
    let id = work_id(raw_id)?;
    ensure_work(&state, id).await?;
    let rating =
        RatingRecord::new(id, input.rating, input.aspects).map_err(ApiError::bad_request)?;
    let mut dataset = load_dataset(&state.database).await?;
    let mut ratings = dataset.ratings().to_vec();
    if let Some(existing) = ratings.iter_mut().find(|existing| existing.work_id() == id) {
        *existing = rating.clone();
    } else {
        ratings.push(rating.clone());
    }
    dataset = OfflineDataset::from_parts(
        dataset.catalog().to_vec(),
        ratings,
        dataset.events().to_vec(),
    )
    .map_err(ApiError::bad_request)?;
    let (profile_json, score_json) = calculate_snapshot(&dataset)?;
    let version = state
        .database
        .snapshots()
        .create_for_rating(&rating, (state.clock)(), &profile_json, &score_json)
        .await?;
    Ok(Json(json!({ "profile_version": version })))
}

async fn append_event(
    State(state): State<ApiState>,
    Path(raw_id): Path<u32>,
    Json(event): Json<WatchEvent>,
) -> Result<StatusCode, ApiError> {
    let id = work_id(raw_id)?;
    if event.work_id() != id {
        return Err(ApiError::bad_request("path and event identifiers differ"));
    }
    ensure_work(&state, id).await?;
    state.database.events().append(&event).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn profile(State(state): State<ApiState>) -> Result<Json<ProfileSnapshot>, ApiError> {
    state
        .database
        .snapshots()
        .latest_profile()
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("profile has not been calculated"))
}

async fn profiles(State(state): State<ApiState>) -> Result<Json<Vec<ProfileSnapshot>>, ApiError> {
    Ok(Json(state.database.snapshots().profiles().await?))
}

#[derive(Deserialize)]
struct RecommendationFeedback {
    helpful: bool,
}

async fn recommendation_feedback(
    State(state): State<ApiState>,
    Path(raw_id): Path<u32>,
    Json(feedback): Json<RecommendationFeedback>,
) -> Result<StatusCode, ApiError> {
    let id = work_id(raw_id)?;
    ensure_work(&state, id).await?;
    state
        .database
        .preferences()
        .set(
            &format!("recommendation_feedback:{}", id.get()),
            &json!({ "helpful": feedback.helpful, "created_at_unix": (state.clock)() }),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn recommendations(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    let snapshot = state
        .database
        .snapshots()
        .latest_profile()
        .await?
        .ok_or_else(|| ApiError::not_found("profile has not been calculated"))?;
    let scores = state.database.snapshots().scores(snapshot.version).await?;
    Ok(Json(
        json!({ "profile_version": snapshot.version, "recommendations": scores }),
    ))
}

async fn historical_recommendations(
    State(state): State<ApiState>,
    Path(version): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let scores = state.database.snapshots().scores(version).await?;
    if scores.is_empty() {
        return Err(ApiError::not_found(
            "profile version has no recommendations",
        ));
    }
    Ok(Json(
        json!({ "profile_version": version, "recommendations": scores }),
    ))
}

async fn evaluation(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    let dataset = load_dataset(&state.database).await?;
    let report = evaluate_baselines(&dataset).map_err(ApiError::bad_request)?;
    Ok(Json(
        serde_json::to_value(report).map_err(ApiError::internal)?,
    ))
}

fn calculate_snapshot(dataset: &OfflineDataset) -> Result<(Value, Vec<Value>), ApiError> {
    let profile = build_taste_profile(dataset, &TasteProfileConfig::default())
        .map_err(ApiError::bad_request)?;
    let rated = dataset
        .ratings()
        .iter()
        .map(RatingRecord::work_id)
        .collect::<HashSet<_>>();
    let candidates = dataset
        .catalog()
        .iter()
        .filter(|work| !rated.contains(&work.id()))
        .cloned()
        .collect::<Vec<_>>();
    let scores = RecommendationEngine::default()
        .score_candidates(&profile, &candidates)
        .map_err(ApiError::internal)?;
    let profile_json = serde_json::to_value(&profile).map_err(ApiError::internal)?;
    let score_json = scores
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::internal)?;
    Ok((profile_json, score_json))
}

async fn load_dataset(database: &Database) -> Result<OfflineDataset, ApiError> {
    let works = database.works().all().await?;
    let mut ratings = Vec::new();
    let mut events = Vec::new();
    for work in &works {
        if let Some(rating) = database.ratings().get(work.id()).await? {
            ratings.push(rating);
        }
        events.extend(database.events().for_work(work.id()).await?);
    }
    OfflineDataset::from_parts(works, ratings, events).map_err(ApiError::bad_request)
}

async fn ensure_work(state: &ApiState, id: WorkId) -> Result<(), ApiError> {
    state
        .database
        .works()
        .get(id)
        .await?
        .map(|_| ())
        .ok_or_else(|| ApiError::not_found("work not found"))
}

fn work_id(value: u32) -> Result<WorkId, ApiError> {
    WorkId::new(value).map_err(ApiError::bad_request)
}
fn normalize_comment(comment: Option<String>) -> Option<String> {
    comment
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    #[allow(clippy::needless_pass_by_value)]
    fn bad_request(error: impl ToString) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
        }
    }
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
    #[allow(clippy::needless_pass_by_value)]
    fn internal(error: impl ToString) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}
impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        Self::internal(error)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
