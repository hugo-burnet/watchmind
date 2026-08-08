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
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use watchmind_infrastructure::{
    AniListCatalog, Database, Impression, LibraryEntry, ProfileSnapshot, StorageError,
};
use watchmind_recommendation::{
    AspectCredit, CandidateRequest, FullEvaluationConfig, OfflineDataset, Rating, RatingRecord,
    TasteProfileConfig, TemporalRating, WatchEvent, WorkId, build_taste_profile, evaluate_full,
    evaluate_pipeline_with_request, rank_candidates_fused,
};

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
type SearchFuture = Pin<Box<dyn Future<Output = Result<CatalogResponse, String>> + Send>>;
type Search = Arc<dyn Fn(String, u32, u8, u64) -> SearchFuture + Send + Sync>;
type Discover = Arc<dyn Fn(u32, u8, u64) -> SearchFuture + Send + Sync>;
type TaggedDiscover = Arc<dyn Fn(String, u32, u8, u64) -> SearchFuture + Send + Sync>;
type BandDiscover = Arc<dyn Fn(u8, u8, u32, u8, u64) -> SearchFuture + Send + Sync>;

/// Tranches de note échantillonnées pour constituer le vivier de candidats.
///
/// Les requêtes de découverte trient par note décroissante : n'en prendre que
/// les premières pages produit un vivier déjà classé par le critère auquel on
/// compare ensuite le moteur, qui ne peut donc pas s'en distinguer. Couvrir
/// aussi les tranches basses met dans le pool des œuvres que l'utilisateur
/// rejetterait, ce qui redonne du sens au classement. Le filtrage qualité, s'il
/// est souhaité, appartient à `CandidateRequest::minimum_global_score`, pas au
/// retrieval.
const DISCOVERY_BANDS: [(u8, u8); 5] = [(84, 101), (74, 85), (64, 75), (49, 65), (0, 50)];
const DISCOVERY_BAND_PAGES: u32 = 2;

#[derive(Debug, Clone, Serialize)]
pub struct CatalogResponse {
    pub works: Vec<watchmind_recommendation::NormalizedWork>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CatalogManifest {
    source: &'static str,
    generated_at_unix: u64,
    discovery_tags: Vec<String>,
    work_ids: Vec<WorkId>,
}

#[derive(Clone)]
pub struct ApiState {
    database: Database,
    search: Search,
    discover: Discover,
    discover_by_tag: TaggedDiscover,
    discover_in_band: BandDiscover,
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
        let discovery_catalog = Arc::clone(&catalog);
        let band_catalog = Arc::clone(&catalog);
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
                let catalog = Arc::clone(&discovery_catalog);
                Box::pin(async move {
                    catalog
                        .discover(page, per_page, now)
                        .await
                        .map(|result| catalog_response(&result))
                        .map_err(|error| error.to_string())
                })
            }),
            discover_by_tag: Arc::new(move |tag, page, per_page, now| {
                let catalog = Arc::clone(&catalog);
                Box::pin(async move {
                    catalog
                        .discover_by_tag(&tag, page, per_page, now)
                        .await
                        .map(|result| catalog_response(&result))
                        .map_err(|error| error.to_string())
                })
            }),
            discover_in_band: Arc::new(move |minimum, maximum, page, per_page, now| {
                let catalog = Arc::clone(&band_catalog);
                Box::pin(async move {
                    catalog
                        .discover_in_band(minimum, maximum, page, per_page, now)
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
        let tagged_discovery = Arc::clone(&search);
        let band_discovery = Arc::clone(&search);
        Self {
            database,
            search: Arc::new(move |query, page, per_page, now| {
                Box::pin(search(query, page, per_page, now))
            }),
            discover: Arc::new(move |page, per_page, now| {
                Box::pin(discovery("__discover__".to_owned(), page, per_page, now))
            }),
            discover_by_tag: Arc::new(move |tag, page, per_page, now| {
                Box::pin(tagged_discovery(tag, page, per_page, now))
            }),
            discover_in_band: Arc::new(move |minimum, maximum, page, per_page, now| {
                Box::pin(band_discovery(
                    format!("__discover_band__:{minimum}:{maximum}"),
                    page,
                    per_page,
                    now,
                ))
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
        .route("/api/recommendations/impact", get(recommendation_impact))
        .route("/api/health", get(health))
        .route("/api/export", get(export_library))
        .route(
            "/api/database",
            get(export_database)
                .put(import_database)
                .delete(clear_database),
        )
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024))
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

async fn export_database(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let body = state.database.export_bytes().await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=watchmind-backup.json",
            ),
        ],
        body,
    )
        .into_response())
}

async fn import_database(
    State(state): State<ApiState>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    state.database.restore_bytes(&body).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn clear_database(State(state): State<ApiState>) -> Result<StatusCode, ApiError> {
    state.database.clear().await?;
    Ok(StatusCode::NO_CONTENT)
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
    let rated_at_unix = (state.clock)();
    let rating = RatingRecord::new(id, input.rating, input.aspects)
        .map_err(ApiError::bad_request)?
        .with_rated_at_unix(rated_at_unix);
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
        .create_for_rating(&rating, rated_at_unix, &profile_json, &score_json)
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
    let personal = load_dataset(&state.database).await?;
    let personal_ids = personal
        .catalog()
        .iter()
        .map(watchmind_recommendation::NormalizedWork::id)
        .collect::<HashSet<_>>();
    let discovery_tags = discovery_tags(&personal)?;
    let visible_discovered = match discover_unseen(&state, &personal_ids, &discovery_tags).await {
        Ok(result) => result,
        Err(_) if current.is_some() => {
            let snapshot = current.expect("checked above");
            return Ok(Json(json!({
                "profile_version": snapshot.version,
                "recommendations": current_scores
            })));
        }
        Err(error) => return Err(ApiError::internal(error)),
    };
    let manifest = catalog_manifest(&state, &discovery_tags, &personal_ids, &visible_discovered);
    state
        .database
        .preferences()
        .set(
            "latest_catalog_manifest",
            &serde_json::to_value(&manifest).map_err(ApiError::internal)?,
        )
        .await?;
    for work in &visible_discovered {
        state.database.works().upsert(work).await?;
    }
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
    record_impressions(&state, version, &scores).await?;
    Ok(Json(json!({
        "profile_version": version,
        "catalog_manifest": manifest,
        "recommendations": scores
    })))
}

/// Nombre de recommandations de tête considérées comme réellement affichées.
const IMPRESSION_DEPTH: usize = 10;

/// Journalise ce qui part vers l'utilisateur.
///
/// On n'enregistre que la tête de liste : au-delà, l'utilisateur ne voit rien,
/// et compter des recommandations invisibles fausserait toute mesure d'apport.
async fn record_impressions(
    state: &ApiState,
    version: i64,
    scores: &[Value],
) -> Result<(), ApiError> {
    let shown_at_unix = (state.clock)();
    let impressions = scores
        .iter()
        .take(IMPRESSION_DEPTH)
        .enumerate()
        .filter_map(|(index, score)| {
            let raw = u32::try_from(score.get("work_id")?.as_u64()?).ok()?;
            Some(Impression {
                work_id: WorkId::new(raw).ok()?,
                profile_version: version,
                shown_at_unix,
                rank: u32::try_from(index + 1).ok()?,
                global_score: score.get("global_score").and_then(Value::as_f64),
            })
        })
        .collect::<Vec<_>>();
    state.database.impressions().record(&impressions).await?;
    Ok(())
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

/// Mesure l'apport réel du moteur, à partir de ce qui a été affiché.
///
/// Le leave-one-out ne peut pas répondre à cette question : il oppose une œuvre
/// que l'utilisateur a choisie et adorée à des œuvres qu'il n'a jamais touchées,
/// si bien qu'un simple tri par notoriété gagne d'avance. Ici, on ne compte que
/// les œuvres notées **après** avoir été recommandées.
async fn recommendation_impact(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    let impressions = state.database.impressions().all().await?;
    let dataset = load_dataset(&state.database).await?;
    let ratings = dataset
        .ratings()
        .iter()
        .map(|rating| (rating.work_id(), rating))
        .collect::<std::collections::HashMap<_, _>>();
    let global = dataset
        .catalog()
        .iter()
        .map(|work| (work.id(), work.global_score().map(Rating::get)))
        .collect::<std::collections::HashMap<_, _>>();

    let mut followed = Vec::new();
    for impression in &impressions {
        let Some(rating) = ratings.get(&impression.work_id) else {
            continue;
        };
        // Seule une note posée après l'affichage atteste d'un visionnage
        // déclenché par la recommandation.
        if rating
            .rated_at_unix()
            .is_none_or(|rated_at| rated_at < impression.shown_at_unix)
        {
            continue;
        }
        let reference = global
            .get(&impression.work_id)
            .copied()
            .flatten()
            .or(impression.global_score);
        followed.push((rating.rating().get(), reference));
    }

    #[allow(clippy::cast_precision_loss)]
    let ratio = |count: usize, total: usize| {
        if total == 0 {
            Value::Null
        } else {
            json!(count as f64 / total as f64)
        }
    };
    let liked = followed.iter().filter(|(rating, _)| *rating >= 8.0).count();
    let above_global = followed
        .iter()
        .filter(|(rating, reference)| reference.is_some_and(|reference| *rating > reference))
        .count();
    #[allow(clippy::cast_precision_loss)]
    let shown_mean_global = {
        let scores = impressions
            .iter()
            .filter_map(|impression| impression.global_score)
            .collect::<Vec<_>>();
        if scores.is_empty() {
            Value::Null
        } else {
            json!(scores.iter().sum::<f64>() / scores.len() as f64)
        }
    };

    Ok(Json(json!({
        "shown": impressions.len(),
        "distinct_works": impressions
            .iter()
            .map(|impression| impression.work_id)
            .collect::<HashSet<_>>()
            .len(),
        "watched_after_being_shown": followed.len(),
        "liked": liked,
        "precision": ratio(liked, followed.len()),
        "above_global_score": above_global,
        "discovery_precision": ratio(above_global, followed.len()),
        "shown_mean_global_score": shown_mean_global
    })))
}

async fn evaluation(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    let personal = load_dataset(&state.database).await?;
    let personal_ids = personal
        .catalog()
        .iter()
        .map(watchmind_recommendation::NormalizedWork::id)
        .collect::<HashSet<_>>();
    let discovery_tags = discovery_tags(&personal)?;
    let discovered = discover_unseen(&state, &personal_ids, &discovery_tags)
        .await
        .map_err(ApiError::internal)?;
    let manifest = catalog_manifest(&state, &discovery_tags, &personal_ids, &discovered);
    let mut catalog = personal.catalog().to_vec();
    catalog.extend(discovered);
    let dataset = OfflineDataset::from_parts(
        catalog,
        personal.ratings().to_vec(),
        personal.events().to_vec(),
    )
    .map_err(ApiError::bad_request)?;
    let temporal_ratings = state
        .database
        .ratings()
        .dated()
        .await?
        .into_iter()
        .map(|(work_id, timestamp)| TemporalRating::new(work_id, unix_date(timestamp)))
        .collect();
    let evaluation_config = FullEvaluationConfig::default().with_temporal_ratings(temporal_ratings);
    let report = evaluate_full(&dataset, &evaluation_config).map_err(ApiError::bad_request)?;
    let reserve_sweep = [0.0, 0.1, 0.25, 0.5]
        .into_iter()
        .map(|popularity_reserve| {
            let request: CandidateRequest = serde_json::from_value(json!({
                "popularity_reserve": popularity_reserve
            }))
            .map_err(ApiError::internal)?;
            let pipeline = evaluate_pipeline_with_request(&dataset, 8.0, &request)
                .map_err(ApiError::bad_request)?;
            Ok(json!({
                "popularity_reserve": popularity_reserve,
                "pipeline": pipeline
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let mut payload = serde_json::to_value(report.baselines()).map_err(ApiError::internal)?;
    payload
        .as_object_mut()
        .expect("an evaluation report always serializes as an object")
        .insert(
            "engine".to_owned(),
            serde_json::to_value(report.engine()).map_err(ApiError::internal)?,
        );
    payload
        .as_object_mut()
        .expect("an evaluation report always serializes as an object")
        .insert(
            "pipeline".to_owned(),
            serde_json::to_value(report.pipeline()).map_err(ApiError::internal)?,
        );
    let object = payload
        .as_object_mut()
        .expect("an evaluation report always serializes as an object");
    object.insert(
        "catalog_manifest".to_owned(),
        serde_json::to_value(manifest).map_err(ApiError::internal)?,
    );
    object.insert("popularity_reserve_sweep".to_owned(), json!(reserve_sweep));
    object.insert(
        "temporal_backtest".to_owned(),
        serde_json::to_value(report.temporal_backtest()).map_err(ApiError::internal)?,
    );
    Ok(Json(payload))
}

fn catalog_manifest(
    state: &ApiState,
    discovery_tags: &[String],
    personal_ids: &HashSet<WorkId>,
    works: &[watchmind_recommendation::NormalizedWork],
) -> CatalogManifest {
    let mut work_ids = personal_ids
        .iter()
        .copied()
        .chain(
            works
                .iter()
                .map(watchmind_recommendation::NormalizedWork::id),
        )
        .collect::<Vec<_>>();
    work_ids.sort_unstable();
    CatalogManifest {
        source: "anilist_live",
        generated_at_unix: (state.clock)(),
        discovery_tags: discovery_tags.to_vec(),
        work_ids,
    }
}

fn unix_date(timestamp: u64) -> String {
    let days = i64::try_from(timestamp / 86_400).unwrap_or(i64::MAX);
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

async fn discover_unseen(
    state: &ApiState,
    excluded: &HashSet<WorkId>,
    tags: &[String],
) -> Result<Vec<watchmind_recommendation::NormalizedWork>, String> {
    let mut works = Vec::new();
    let mut seen = excluded.clone();
    let mut queries = vec![(None, 1), (None, 2)];
    for tag in tags {
        queries.push((Some(tag.as_str()), 1));
        queries.push((Some(tag.as_str()), 2));
    }

    let mut responses = Vec::new();
    for (tag, page) in queries {
        responses.push(match tag {
            Some(tag) => (state.discover_by_tag)(tag.to_owned(), page, 50, (state.clock)()).await,
            None => (state.discover)(page, 50, (state.clock)()).await,
        });
    }
    // Échantillonnage par tranche de note : sans lui le vivier ne contient que
    // le sommet du palmarès, et un tri par note mondiale y devient un oracle.
    for (minimum, maximum) in DISCOVERY_BANDS {
        for page in 1..=DISCOVERY_BAND_PAGES {
            responses
                .push((state.discover_in_band)(minimum, maximum, page, 50, (state.clock)()).await);
        }
    }

    for response in responses {
        let response = match response {
            Ok(response) => response,
            Err(error) if works.is_empty() => return Err(error),
            Err(_) => continue,
        };
        for work in response.works {
            if seen.insert(work.id())
                && state
                    .database
                    .preferences()
                    .get(&format!("hidden_work:{}", work.id().get()))
                    .await
                    .map_err(|error| error.to_string())?
                    .is_none()
            {
                works.push(work);
            }
        }
    }
    Ok(works)
}

fn discovery_tags(dataset: &OfflineDataset) -> Result<Vec<String>, ApiError> {
    if dataset.ratings().is_empty() {
        return Ok(Vec::new());
    }
    let profile = build_taste_profile(dataset, &TasteProfileConfig::default())
        .map_err(ApiError::bad_request)?;
    let mut tags = profile
        .tag_affinities()
        .iter()
        .filter(|tag| tag.value() > 0.0)
        .map(|tag| (tag.name().to_owned(), tag.value() * tag.confidence().get()))
        .collect::<Vec<_>>();
    tags.sort_by(|(left_name, left), (right_name, right)| {
        right
            .total_cmp(left)
            .then_with(|| left_name.cmp(right_name))
    });
    tags.truncate(4);
    Ok(tags.into_iter().map(|(name, _)| name).collect())
}

fn calculate_snapshot(dataset: &OfflineDataset) -> Result<(Value, Vec<Value>), ApiError> {
    if dataset.ratings().is_empty() {
        let mut scores = dataset
            .catalog()
            .iter()
            .filter_map(|work| {
                let global_score = work.global_score()?.get();
                let value = (global_score - 5.0) / 50.0;
                let qualifier = if value < 0.0 { "faible" } else { "élevé" };
                let contribution = json!({
                    "source": { "kind": "anilist_prior" },
                    "value": value,
                    "detail": format!("Prior AniList {qualifier} ({global_score:.1}/10)")
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
    let scores =
        rank_candidates_fused(dataset, &profile, &candidates).map_err(ApiError::internal)?;
    let profile_json = serde_json::to_value(&profile).map_err(ApiError::internal)?;
    let score_json = scores
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::internal)?;
    Ok((profile_json, score_json))
}

/// Remplace les données personnelles par un dataset et crée son profil initial.
/// # Errors
/// Retourne un diagnostic si l'import, le calcul ou la persistance échoue.
pub async fn replace_with_dataset(
    database: &Database,
    dataset: &OfflineDataset,
) -> Result<(), String> {
    database
        .replace_with_dataset(dataset)
        .await
        .map_err(|error| error.to_string())?;
    let (profile, scores) = calculate_snapshot(dataset).map_err(|error| error.message)?;
    let created_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs());
    database
        .snapshots()
        .create(created_at_unix, &profile, &scores)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::unix_date;

    #[test]
    fn converts_unix_days_to_iso_dates() {
        assert_eq!(unix_date(0), "1970-01-01");
        assert_eq!(unix_date(1_700_000_000), "2023-11-14");
        assert_eq!(unix_date(1_772_323_200), "2026-03-01");
    }
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
