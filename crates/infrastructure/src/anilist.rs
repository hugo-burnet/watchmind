use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use watchmind_recommendation::{
    DomainError, NormalizedWork, Rating, ReleaseYear, RuntimeMinutes, TagWeight, Weight,
    WorkFormat, WorkId,
};

const SEARCH_QUERY: &str = r"query SearchAnime($search: String!, $page: Int!, $perPage: Int!) {
  Page(page: $page, perPage: $perPage) {
    media(search: $search, type: ANIME, sort: SEARCH_MATCH) {
      id title { userPreferred romaji english native } averageScore duration episodes format
      startDate { year } tags { name rank isMediaSpoiler } studios(isMain: true) { nodes { name } }
    }
  }
}";

const DISCOVER_QUERY: &str = r"query DiscoverAnime($page: Int!, $perPage: Int!) {
  Page(page: $page, perPage: $perPage) {
    media(type: ANIME, sort: [SCORE_DESC, POPULARITY_DESC], status: FINISHED) {
      id title { userPreferred romaji english native } averageScore duration episodes format
      startDate { year } tags { name rank isMediaSpoiler } studios(isMain: true) { nodes { name } }
    }
  }
}";

#[derive(Debug, thiserror::Error)]
pub enum AniListError {
    #[error("AniList request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("catalog cache I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid AniList payload: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid normalized work: {0}")]
    Domain(#[from] DomainError),
    #[error("AniList returned GraphQL errors: {0}")]
    GraphQl(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    works: Vec<NormalizedWork>,
    from_cache: bool,
}

impl SearchResult {
    #[must_use]
    pub fn works(&self) -> &[NormalizedWork] {
        &self.works
    }
    #[must_use]
    pub const fn from_cache(&self) -> bool {
        self.from_cache
    }
}

/// Normalise une réponse `AniList` sans effectuer d'accès réseau.
pub struct AniListNormalizer;

impl AniListNormalizer {
    /// # Errors
    /// Refuse une réponse GraphQL en erreur ou un contrat métier invalide.
    pub fn normalize(payload: &str) -> Result<Vec<NormalizedWork>, AniListError> {
        let response: GraphQlResponse = serde_json::from_str(payload)?;
        if !response.errors.is_empty() {
            return Err(AniListError::GraphQl(
                response
                    .errors
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        response.data.map_or_else(
            || Ok(Vec::new()),
            |data| data.page.media.into_iter().map(normalize_media).collect(),
        )
    }
}

/// Cache de réponses GraphQL brutes, relisibles et renormalisables hors ligne.
#[derive(Debug, Clone)]
pub struct CatalogCache {
    directory: PathBuf,
    max_age: Duration,
}

impl CatalogCache {
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>, max_age: Duration) -> Self {
        Self {
            directory: directory.into(),
            max_age,
        }
    }

    fn path_for(&self, search: &str, page: u32, per_page: u8) -> PathBuf {
        let mut hash = Sha256::new();
        hash.update(search.trim().to_lowercase());
        hash.update(page.to_le_bytes());
        hash.update([per_page]);
        self.directory.join(format!("{:x}.json", hash.finalize()))
    }

    async fn read(&self, key: &Path, now_unix: u64) -> Result<Option<String>, AniListError> {
        let text = match tokio::fs::read_to_string(key).await {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let entry: CacheEntry = serde_json::from_str(&text)?;
        if now_unix.saturating_sub(entry.fetched_at_unix) > self.max_age.as_secs() {
            return Ok(None);
        }
        Ok(Some(entry.payload))
    }

    async fn write(
        &self,
        key: &Path,
        fetched_at_unix: u64,
        payload: &str,
    ) -> Result<(), AniListError> {
        tokio::fs::create_dir_all(&self.directory).await?;
        let entry = CacheEntry {
            fetched_at_unix,
            payload: payload.to_owned(),
        };
        let temporary = key.with_extension("tmp");
        tokio::fs::write(&temporary, serde_json::to_vec_pretty(&entry)?).await?;
        tokio::fs::rename(temporary, key).await?;
        Ok(())
    }
}

/// Client GraphQL `AniList` avec cache local obligatoire.
pub struct AniListCatalog {
    client: Client,
    endpoint: String,
    cache: CatalogCache,
}

impl AniListCatalog {
    #[must_use]
    pub fn new(cache: CatalogCache) -> Self {
        Self::with_endpoint(cache, "https://graphql.anilist.co")
    }

    #[must_use]
    pub fn with_endpoint(cache: CatalogCache, endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.into(),
            cache,
        }
    }

    /// Recherche une page (1 à 50 résultats), puis la met en cache.
    ///
    /// # Errors
    /// Retourne les erreurs réseau, cache, JSON ou de normalisation.
    pub async fn search(
        &self,
        search: &str,
        page: u32,
        per_page: u8,
        now_unix: u64,
    ) -> Result<SearchResult, AniListError> {
        let per_page = per_page.clamp(1, 50);
        let page = page.max(1);
        let key = self.cache.path_for(search, page, per_page);
        if let Some(payload) = self.cache.read(&key, now_unix).await? {
            return Ok(SearchResult {
                works: AniListNormalizer::normalize(&payload)?,
                from_cache: true,
            });
        }
        let body = GraphQlRequest {
            query: SEARCH_QUERY,
            variables: Variables {
                search,
                page,
                per_page,
            },
        };
        let payload = self
            .client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let works = AniListNormalizer::normalize(&payload)?;
        self.cache.write(&key, now_unix, &payload).await?;
        Ok(SearchResult {
            works,
            from_cache: false,
        })
    }

    /// Charge une page de candidats populaires et bien notés, indépendamment
    /// de la bibliothèque personnelle.
    /// # Errors
    /// Retourne les erreurs réseau, cache, JSON ou de normalisation.
    pub async fn discover(
        &self,
        page: u32,
        per_page: u8,
        now_unix: u64,
    ) -> Result<SearchResult, AniListError> {
        let per_page = per_page.clamp(1, 50);
        let page = page.max(1);
        let key = self.cache.path_for("__discover__", page, per_page);
        if let Some(payload) = self.cache.read(&key, now_unix).await? {
            return Ok(SearchResult {
                works: AniListNormalizer::normalize(&payload)?,
                from_cache: true,
            });
        }
        let payload = self
            .client
            .post(&self.endpoint)
            .json(&DiscoverGraphQlRequest {
                query: DISCOVER_QUERY,
                variables: DiscoverVariables { page, per_page },
            })
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let works = AniListNormalizer::normalize(&payload)?;
        self.cache.write(&key, now_unix, &payload).await?;
        Ok(SearchResult {
            works,
            from_cache: false,
        })
    }
}

fn normalize_media(media: Media) -> Result<NormalizedWork, AniListError> {
    let id = WorkId::new(media.id)?;
    let title = media
        .title
        .user_preferred
        .or(media.title.english)
        .or(media.title.romaji)
        .or(media.title.native)
        .unwrap_or_else(|| format!("AniList #{}", media.id));
    let score = media
        .average_score
        .map(|value| Rating::new(f64::from(value) / 10.0))
        .transpose()?;
    let tags = media
        .tags
        .into_iter()
        .filter(|tag| !tag.is_media_spoiler && tag.rank > 0)
        .map(|tag| TagWeight::new(tag.name, Weight::new(f64::from(tag.rank) / 100.0)?))
        .collect::<Result<Vec<_>, DomainError>>()?;
    let mut work = NormalizedWork::new(id, title, score, tags)?;
    if let (Some(duration), Some(episodes)) = (media.duration, media.episodes) {
        if let Some(total) = duration.checked_mul(episodes).filter(|value| *value > 0) {
            work = work.with_runtime_minutes(RuntimeMinutes::new(total)?);
        }
    } else if let Some(duration) = media.duration.filter(|value| *value > 0) {
        work = work.with_runtime_minutes(RuntimeMinutes::new(duration)?);
    }
    if let Some(format) = media.format.as_deref().and_then(map_format) {
        work = work.with_format(format);
    }
    if let Some(year) = media
        .start_date
        .year
        .and_then(|year| u16::try_from(year).ok())
        .filter(|year| *year >= 1900)
    {
        work = work.with_release_year(ReleaseYear::new(year)?);
    }
    work = work.with_studios(
        media
            .studios
            .nodes
            .into_iter()
            .map(|studio| studio.name)
            .collect(),
    )?;
    Ok(work)
}

fn map_format(value: &str) -> Option<WorkFormat> {
    match value {
        "TV" | "TV_SHORT" => Some(WorkFormat::Tv),
        "MOVIE" => Some(WorkFormat::Movie),
        "OVA" => Some(WorkFormat::Ova),
        "ONA" => Some(WorkFormat::Ona),
        "SPECIAL" => Some(WorkFormat::Special),
        "MUSIC" => Some(WorkFormat::Music),
        _ => None,
    }
}

#[derive(Serialize)]
struct GraphQlRequest<'a> {
    query: &'static str,
    variables: Variables<'a>,
}
#[derive(Serialize)]
struct DiscoverGraphQlRequest {
    query: &'static str,
    variables: DiscoverVariables,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverVariables {
    page: u32,
    per_page: u8,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Variables<'a> {
    search: &'a str,
    page: u32,
    per_page: u8,
}
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    fetched_at_unix: u64,
    payload: String,
}
#[derive(Deserialize)]
struct GraphQlResponse {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}
#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}
#[derive(Deserialize)]
struct Data {
    #[serde(rename = "Page")]
    page: Page,
}
#[derive(Deserialize)]
struct Page {
    media: Vec<Media>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Media {
    id: u32,
    title: Titles,
    average_score: Option<u8>,
    duration: Option<u32>,
    episodes: Option<u32>,
    format: Option<String>,
    start_date: StartDate,
    tags: Vec<ApiTag>,
    studios: Studios,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Titles {
    user_preferred: Option<String>,
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}
#[derive(Deserialize)]
struct StartDate {
    year: Option<i32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiTag {
    name: String,
    rank: u8,
    is_media_spoiler: bool,
}
#[derive(Deserialize)]
struct Studios {
    nodes: Vec<Studio>,
}
#[derive(Deserialize)]
struct Studio {
    name: String,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::CatalogCache;

    #[tokio::test]
    async fn cache_honors_expiration() {
        let directory = tempfile::tempdir().unwrap();
        let cache = CatalogCache::new(directory.path(), Duration::from_mins(1));
        let key = cache.path_for("Death Note", 1, 10);
        cache.write(&key, 1_000, "payload").await.unwrap();

        assert_eq!(
            cache.read(&key, 1_060).await.unwrap().as_deref(),
            Some("payload")
        );
        assert_eq!(cache.read(&key, 1_061).await.unwrap(), None);
    }
}
