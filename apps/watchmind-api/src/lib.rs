//! API HTTP locale de `WatchMind`.

use std::{
    collections::HashSet,
    fmt::Write as _,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
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
type Discover = Arc<dyn Fn(u32, u8, u64) -> SearchFuture + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
pub struct CatalogResponse {
    pub works: Vec<watchmind_recommendation::NormalizedWork>,
    pub from_cache: bool,
}

#[derive(Clone)]
pub struct ApiState {
    database: Database,
    search: Search,
    discover: Discover,
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
        let search_catalog = Arc::clone(&catalog);
        Self {
            database,
            search: Arc::new(move |query, page, per_page, now| {
                let catalog = Arc::clone(&search_catalog);
                Box::pin(async move {
                    catalog
                        .search(&query, page, per_page, now)
                        .await
                        .map(|result| catalog_response(&result))
                        .map_err(|error| error.to_string())
                })
            }),
            discover: Arc::new(move |page, per_page, now| {
                let catalog = Arc::clone(&catalog);
                Box::pin(async move {
                    catalog
                        .discover(page, per_page, now)
                        .await
                        .map(|result| catalog_response(&result))
                        .map_err(|error| error.to_string())
                })
            }),
            clock: Arc::new(clock),
        }
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
        let search = Arc::new(search);
        let discovery = Arc::clone(&search);
        Self {
            database,
            search: Arc::new(move |query, page, per_page, now| {
                Box::pin(search(query, page, per_page, now))
            }),
            discover: Arc::new(move |page, per_page, now| {
                Box::pin(discovery("__discover__".to_owned(), page, per_page, now))
            }),
            clock: Arc::new(clock),
        }
    }
}

fn catalog_response(result: &watchmind_infrastructure::SearchResult) -> CatalogResponse {
    CatalogResponse {
        works: result.works().to_vec(),
        from_cache: result.from_cache(),
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/anime/search", get(search_anime))
        .route("/api/works/{id}", get(read_work))
        .route("/api/library", get(library))
        .route(
            "/api/library/{id}",
            put(upsert_library).delete(remove_library),
        )
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
        .route("/api/health", get(health))
        .route("/api/export", get(export_library))
        .with_state(state)
}

pub fn secured_router(state: ApiState, token: Option<String>) -> Router {
    let app = router(state);
    token.map_or(app.clone(), |token| {
        app.layer(middleware::from_fn_with_state(token, require_token))
    })
}

async fn require_token(
    State(token): State<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let expected = format!("Bearer {token}");
    if request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(expected.as_str())
    {
        next.run(request).await
    } else {
        ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "missing or invalid bearer token".to_owned(),
        }
        .into_response()
    }
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default = "json_format")]
    format: String,
}

fn json_format() -> String {
    "json".to_owned()
}

async fn export_library(
    State(state): State<ApiState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let entries = complete_library(&state).await?;
    let (content_type, body) = match query.format.as_str() {
        "json" => (
            "application/json; charset=utf-8",
            serde_json::to_string_pretty(&entries).map_err(ApiError::internal)?,
        ),
        "csv" => ("text/csv; charset=utf-8", library_csv(&entries)),
        "markdown" | "md" => ("text/markdown; charset=utf-8", library_markdown(&entries)),
        _ => {
            return Err(ApiError::bad_request(
                "format must be json, csv or markdown",
            ));
        }
    };
    Ok(([(header::CONTENT_TYPE, content_type)], body).into_response())
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
    Ok(Json(complete_library(&state).await?))
}

async fn complete_library(state: &ApiState) -> Result<Vec<CompleteWork>, ApiError> {
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
    Ok(result)
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn library_csv(entries: &[CompleteWork]) -> String {
    let mut output = "id,title,rating,status,comment\n".to_owned();
    for entry in entries {
        let status = entry.events.last().map_or("en_cours", |event| match event {
            WatchEvent::Completed { .. } => "termine",
            WatchEvent::Dropped { .. } => "arrete",
            WatchEvent::Rewatched { .. } => "rewatch",
        });
        writeln!(
            output,
            "{},{},{},{},{}",
            entry.work.id().get(),
            csv_cell(entry.work.title()),
            entry
                .rating
                .as_ref()
                .map_or_else(String::new, |rating| rating.rating().get().to_string()),
            status,
            csv_cell(
                entry
                    .library
                    .as_ref()
                    .and_then(|library| library.comment.as_deref())
                    .unwrap_or_default()
            )
        )
        .expect("writing to a String cannot fail");
    }
    output
}

fn library_markdown(entries: &[CompleteWork]) -> String {
    let mut output =
        "# Bibliothèque WatchMind\n\n| Œuvre | Note | Commentaire |\n| --- | ---: | --- |\n"
            .to_owned();
    for entry in entries {
        writeln!(
            output,
            "| {} | {} | {} |",
            entry.work.title().replace('|', "\\|"),
            entry.rating.as_ref().map_or_else(
                || "—".to_owned(),
                |rating| format!("{}/10", rating.rating().get())
            ),
            entry
                .library
                .as_ref()
                .and_then(|library| library.comment.as_deref())
                .unwrap_or_default()
                .replace('|', "\\|")
        )
        .expect("writing to a String cannot fail");
    }
    output
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
    let is_new = state.database.library().get(id).await?.is_none();
    state.database.works().upsert(&input.work).await?;
    state
        .database
        .library()
        .upsert(&LibraryEntry {
            work_id: id,
            comment: normalize_comment(input.comment),
        })
        .await?;
    if is_new {
        let dataset = load_dataset(&state.database).await?;
        if !dataset.ratings().is_empty() {
            let (profile_json, score_json) = calculate_snapshot(&dataset)?;
            state
                .database
                .snapshots()
                .create((state.clock)(), &profile_json, &score_json)
                .await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_library(
    State(state): State<ApiState>,
    Path(raw_id): Path<u32>,
) -> Result<Json<Value>, ApiError> {
    let id = work_id(raw_id)?;
    state
        .database
        .library()
        .get(id)
        .await?
        .ok_or_else(|| ApiError::not_found("library entry not found"))?;
    let dataset = load_dataset(&state.database).await?;
    let dataset = OfflineDataset::from_parts(
        dataset
            .catalog()
            .iter()
            .filter(|work| work.id() != id)
            .cloned()
            .collect(),
        dataset
            .ratings()
            .iter()
            .filter(|rating| rating.work_id() != id)
            .cloned()
            .collect(),
        dataset
            .events()
            .iter()
            .filter(|event| event.work_id() != id)
            .cloned()
            .collect(),
    )
    .map_err(ApiError::bad_request)?;
    let (profile_json, score_json) = calculate_snapshot(&dataset)?;
    let version = state
        .database
        .snapshots()
        .create_for_removal(id, (state.clock)(), &profile_json, &score_json)
        .await?;
    Ok(Json(json!({ "profile_version": version })))
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
    let current = state.database.snapshots().latest_profile().await?;
    let current_scores = match &current {
        Some(snapshot) => state.database.snapshots().scores(snapshot.version).await?,
        None => Vec::new(),
    };
    let discovered = match (state.discover)(1, 50, (state.clock)()).await {
        Ok(result) => result.works,
        Err(_) if current.is_some() => {
            let snapshot = current.expect("checked above");
            return Ok(Json(json!({
                "profile_version": snapshot.version,
                "recommendations": current_scores
            })));
        }
        Err(error) => return Err(ApiError::internal(error)),
    };
    let mut visible_discovered = Vec::new();
    for work in discovered {
        if state
            .database
            .preferences()
            .get(&format!("hidden_work:{}", work.id().get()))
            .await?
            .is_none()
        {
            visible_discovered.push(work);
        }
    }
    for work in &visible_discovered {
        state.database.works().upsert(work).await?;
    }
    let personal = load_dataset(&state.database).await?;
    let personal_ids = personal
        .catalog()
        .iter()
        .map(watchmind_recommendation::NormalizedWork::id)
        .collect::<HashSet<_>>();
    let mut catalog = personal.catalog().to_vec();
    catalog.extend(
        visible_discovered
            .into_iter()
            .filter(|work| !personal_ids.contains(&work.id())),
    );
    let dataset = OfflineDataset::from_parts(
        catalog,
        personal.ratings().to_vec(),
        personal.events().to_vec(),
    )
    .map_err(ApiError::bad_request)?;
    let (profile, scores) = calculate_snapshot(&dataset)?;
    let version = match current {
        Some(snapshot) if snapshot.profile == profile && current_scores == scores => {
            snapshot.version
        }
        _ => {
            state
                .database
                .snapshots()
                .create((state.clock)(), &profile, &scores)
                .await?
        }
    };
    Ok(Json(
        json!({ "profile_version": version, "recommendations": scores }),
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
    if dataset.ratings().is_empty() {
        let mut scores = dataset
            .catalog()
            .iter()
            .filter_map(|work| {
                let global_score = work.global_score()?.get();
                let value = (global_score - 5.0) / 50.0;
                let contribution = json!({
                    "source": { "kind": "anilist_prior" },
                    "value": value,
                    "detail": format!("Prior AniList faible ({global_score:.1}/10)")
                });
                Some(json!({
                    "work_id": work.id(),
                    "title": work.title(),
                    "score": { "total": value, "contributions": [contribution.clone()] },
                    "explanation": {
                        "reasons": if value > 0.0 { vec![contribution.clone()] } else { Vec::new() },
                        "risks": if value < 0.0 { vec![contribution] } else { Vec::new() }
                    }
                }))
            })
            .collect::<Vec<_>>();
        scores.sort_by(|left, right| {
            right["score"]["total"]
                .as_f64()
                .unwrap_or_default()
                .total_cmp(&left["score"]["total"].as_f64().unwrap_or_default())
        });
        return Ok((
            json!({
                "history_size": 0,
                "confidence": 0.0,
                "mode": "sparse_history",
                "tag_affinities": [],
                "poles": [],
                "axes": {
                    "source": "prior",
                    "observed_works": 0,
                    "weights": [
                        { "axis": "story", "weight": 0.2 },
                        { "axis": "characters", "weight": 0.2 },
                        { "axis": "world_building", "weight": 0.2 },
                        { "axis": "visual_direction", "weight": 0.2 },
                        { "axis": "sound_and_music", "weight": 0.2 }
                    ]
                }
            }),
            scores,
        ));
    }
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
    let mut works = Vec::new();
    for entry in database.library().all().await? {
        if let Some(work) = database.works().get(entry.work_id).await? {
            works.push(work);
        }
    }
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
