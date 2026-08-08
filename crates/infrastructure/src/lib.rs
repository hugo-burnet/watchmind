//! Adaptateurs `AniList` et `SQLite` de `WatchMind`.
//!
//! Cette crate dépend du moteur, jamais l'inverse.

mod anilist;
mod storage;

pub use anilist::{AniListCatalog, AniListError, AniListNormalizer, CatalogCache, SearchResult};
pub use storage::{
    AspectRepository, Database, EventRepository, PreferenceRepository, RatingRepository,
    StorageError, TagRepository, WorkRepository,
};
