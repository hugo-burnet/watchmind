use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    io::Read,
};

use serde::Deserialize;

use crate::{DropProgress, NormalizedWork, Rating, RatingRecord, WatchEvent, WorkId};

const RATINGS_HEADERS: [&str; 6] = [
    "work_id",
    "rating",
    "status",
    "drop_position",
    "total_episodes",
    "rewatches",
];

/// Dataset local validé, prêt à être consommé par le moteur offline.
///
/// L'import est déterministe : le catalogue, les notes et les événements sont
/// ordonnés par identifiant `AniList`, indépendamment de l'ordre des lignes CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct OfflineDataset {
    catalog: Vec<NormalizedWork>,
    ratings: Vec<RatingRecord>,
    events: Vec<WatchEvent>,
}

impl OfflineDataset {
    /// Importe simultanément l'historique CSV et le snapshot catalogue JSON.
    ///
    /// Le CSV doit contenir exactement les colonnes `work_id`, `rating`,
    /// `status`, `drop_position`, `total_episodes` et `rewatches`. `status`
    /// accepte `completed` ou `dropped`. Les deux colonnes de progression sont
    /// obligatoires seulement pour un abandon.
    ///
    /// # Errors
    ///
    /// Retourne un diagnostic localisé si un format est invalide, si une règle
    /// métier est violée, si une œuvre est notée deux fois ou si son identifiant
    /// est absent du catalogue.
    pub fn import(ratings_csv: impl Read, catalog_json: impl Read) -> Result<Self, ImportError> {
        let mut catalog: Vec<NormalizedWork> =
            serde_json::from_reader(catalog_json).map_err(|error| ImportError::CatalogJson {
                line: error.line(),
                column: error.column(),
                reason: error.to_string(),
            })?;
        validate_catalog_ids(&catalog)?;
        catalog.sort_by_key(NormalizedWork::id);

        let catalog_ids: HashSet<_> = catalog.iter().map(NormalizedWork::id).collect();
        let mut entries = import_rating_entries(ratings_csv, &catalog_ids)?;
        entries.sort_by_key(|entry| entry.rating.work_id());

        let mut ratings = Vec::with_capacity(entries.len());
        let mut events = Vec::new();
        for entry in entries {
            ratings.push(entry.rating);
            events.push(entry.status.into_event(entry.work_id));
            events.extend(
                std::iter::repeat_with(|| WatchEvent::rewatched(entry.work_id))
                    .take(entry.rewatches as usize),
            );
        }

        Ok(Self {
            catalog,
            ratings,
            events,
        })
    }

    #[must_use]
    pub fn catalog(&self) -> &[NormalizedWork] {
        &self.catalog
    }

    #[must_use]
    pub fn ratings(&self) -> &[RatingRecord] {
        &self.ratings
    }

    #[must_use]
    pub fn events(&self) -> &[WatchEvent] {
        &self.events
    }

    #[must_use]
    pub fn summary(&self) -> ImportSummary {
        let mut completed = 0;
        let mut dropped = 0;
        let mut rewatches = 0;
        for event in &self.events {
            match event {
                WatchEvent::Completed { .. } => completed += 1,
                WatchEvent::Dropped { .. } => dropped += 1,
                WatchEvent::Rewatched { .. } => rewatches += 1,
            }
        }
        ImportSummary {
            catalog: self.catalog.len(),
            ratings: self.ratings.len(),
            completed,
            dropped,
            rewatches,
        }
    }
}

/// Compteurs stables affichés par la CLI après un import réussi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub catalog: usize,
    pub ratings: usize,
    pub completed: usize,
    pub dropped: usize,
    pub rewatches: usize,
}

impl fmt::Display for ImportSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "import ok: catalog={} ratings={} completed={} dropped={} rewatches={}",
            self.catalog, self.ratings, self.completed, self.dropped, self.rewatches
        )
    }
}

/// Diagnostic d'import précis et exploitable par une CLI ou un autre adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    CatalogJson {
        line: usize,
        column: usize,
        reason: String,
    },
    DuplicateCatalogWork {
        work_id: WorkId,
    },
    InvalidHeaders {
        expected: Vec<String>,
        found: Vec<String>,
    },
    Csv {
        line: Option<u64>,
        reason: String,
    },
    InvalidField {
        line: u64,
        field: &'static str,
        value: String,
        reason: &'static str,
    },
    DuplicateRating {
        work_id: WorkId,
        first_line: u64,
        duplicate_line: u64,
    },
    UnknownWork {
        work_id: WorkId,
        line: u64,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CatalogJson {
                line,
                column,
                reason,
            } => write!(
                formatter,
                "catalog.json at line {line}, column {column}: {reason}"
            ),
            Self::DuplicateCatalogWork { work_id } => {
                write!(
                    formatter,
                    "catalog.json: duplicate work_id {}",
                    work_id.get()
                )
            }
            Self::InvalidHeaders { expected, found } => write!(
                formatter,
                "ratings.csv headers: expected {}, found {}",
                expected.join(","),
                found.join(",")
            ),
            Self::Csv { line, reason } => match line {
                Some(line) => write!(formatter, "ratings.csv at line {line}: {reason}"),
                None => write!(formatter, "ratings.csv: {reason}"),
            },
            Self::InvalidField {
                line,
                field,
                value,
                reason,
            } => write!(
                formatter,
                "ratings.csv at line {line}, field {field}, value {value:?}: {reason}"
            ),
            Self::DuplicateRating {
                work_id,
                first_line,
                duplicate_line,
            } => write!(
                formatter,
                "ratings.csv at line {duplicate_line}: duplicate work_id {} (first seen at line {first_line})",
                work_id.get()
            ),
            Self::UnknownWork { work_id, line } => write!(
                formatter,
                "ratings.csv at line {line}, field work_id, value {:?}: identifier is absent from catalog.json",
                work_id.get().to_string()
            ),
        }
    }
}

impl Error for ImportError {}

#[derive(Debug, Deserialize)]
struct RatingRow {
    work_id: String,
    rating: String,
    status: String,
    drop_position: String,
    total_episodes: String,
    rewatches: String,
}

struct RatingEntry {
    work_id: WorkId,
    rating: RatingRecord,
    status: ViewingStatus,
    rewatches: u32,
}

enum ViewingStatus {
    Completed,
    Dropped(DropProgress),
}

impl ViewingStatus {
    fn into_event(self, work_id: WorkId) -> WatchEvent {
        match self {
            Self::Completed => WatchEvent::completed(work_id),
            Self::Dropped(progress) => WatchEvent::dropped(work_id, progress),
        }
    }
}

fn validate_catalog_ids(catalog: &[NormalizedWork]) -> Result<(), ImportError> {
    let mut ids = HashSet::with_capacity(catalog.len());
    for work in catalog {
        if !ids.insert(work.id()) {
            return Err(ImportError::DuplicateCatalogWork { work_id: work.id() });
        }
    }
    Ok(())
}

fn import_rating_entries(
    ratings_csv: impl Read,
    catalog_ids: &HashSet<WorkId>,
) -> Result<Vec<RatingEntry>, ImportError> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(ratings_csv);
    let header_record = reader.headers().map_err(|error| csv_error(&error))?.clone();
    let headers = header_record.iter().map(str::to_owned).collect::<Vec<_>>();
    let expected = RATINGS_HEADERS.map(str::to_owned).to_vec();
    if headers != expected {
        return Err(ImportError::InvalidHeaders {
            expected,
            found: headers,
        });
    }

    let mut entries = Vec::new();
    let mut seen = HashMap::new();
    for result in reader.records() {
        let record = result.map_err(|error| csv_error(&error))?;
        let line = record.position().map_or(0, csv::Position::line);
        let row = record
            .deserialize::<RatingRow>(Some(&header_record))
            .map_err(|error| csv_error(&error))?;
        let entry = parse_rating_row(&row, line)?;

        if let Some(first_line) = seen.insert(entry.work_id, line) {
            return Err(ImportError::DuplicateRating {
                work_id: entry.work_id,
                first_line,
                duplicate_line: line,
            });
        }
        if !catalog_ids.contains(&entry.work_id) {
            return Err(ImportError::UnknownWork {
                work_id: entry.work_id,
                line,
            });
        }
        entries.push(entry);
    }
    Ok(entries)
}

fn parse_rating_row(row: &RatingRow, line: u64) -> Result<RatingEntry, ImportError> {
    let work_id_value = parse_u32(&row.work_id, line, "work_id")?;
    let work_id = WorkId::new(work_id_value).map_err(|_| ImportError::InvalidField {
        line,
        field: "work_id",
        value: row.work_id.clone(),
        reason: "must be greater than zero",
    })?;

    let rating_value = parse_f64(&row.rating, line, "rating")?;
    let rating = Rating::new(rating_value).map_err(|_| ImportError::InvalidField {
        line,
        field: "rating",
        value: row.rating.clone(),
        reason: "must be a finite number between 0 and 10",
    })?;

    let status = parse_status(row, line)?;
    let rewatches = if row.rewatches.is_empty() {
        0
    } else {
        parse_u32(&row.rewatches, line, "rewatches")?
    };

    Ok(RatingEntry {
        work_id,
        rating: RatingRecord::new(work_id, rating, Vec::new())
            .expect("empty aspects always satisfy the domain invariant"),
        status,
        rewatches,
    })
}

fn parse_status(row: &RatingRow, line: u64) -> Result<ViewingStatus, ImportError> {
    match row.status.as_str() {
        "completed" => {
            if !row.drop_position.is_empty() || !row.total_episodes.is_empty() {
                return Err(ImportError::InvalidField {
                    line,
                    field: "drop_position,total_episodes",
                    value: format!("{},{}", row.drop_position, row.total_episodes),
                    reason: "must both be empty when status is completed",
                });
            }
            Ok(ViewingStatus::Completed)
        }
        "dropped" => {
            if row.drop_position.is_empty() || row.total_episodes.is_empty() {
                return Err(ImportError::InvalidField {
                    line,
                    field: "drop_position,total_episodes",
                    value: format!("{},{}", row.drop_position, row.total_episodes),
                    reason: "must both be provided when status is dropped",
                });
            }
            let position = parse_u32(&row.drop_position, line, "drop_position")?;
            let total = parse_u32(&row.total_episodes, line, "total_episodes")?;
            let progress =
                DropProgress::new(position, total).map_err(|_| ImportError::InvalidField {
                    line,
                    field: "drop_position,total_episodes",
                    value: format!("{position},{total}"),
                    reason: "drop position must be lower than a non-zero total",
                })?;
            Ok(ViewingStatus::Dropped(progress))
        }
        _ => Err(ImportError::InvalidField {
            line,
            field: "status",
            value: row.status.clone(),
            reason: "must be completed or dropped",
        }),
    }
}

fn parse_u32(value: &str, line: u64, field: &'static str) -> Result<u32, ImportError> {
    value.parse::<u32>().map_err(|_| ImportError::InvalidField {
        line,
        field,
        value: value.to_owned(),
        reason: "must be a non-negative integer",
    })
}

fn parse_f64(value: &str, line: u64, field: &'static str) -> Result<f64, ImportError> {
    value.parse::<f64>().map_err(|_| ImportError::InvalidField {
        line,
        field,
        value: value.to_owned(),
        reason: "must be a number",
    })
}

fn csv_error(error: &csv::Error) -> ImportError {
    ImportError::Csv {
        line: error.position().map(csv::Position::line),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = r#"[
        {"id":1,"title":"One","global_score":8.0,"tags":[]},
        {"id":2,"title":"Two","global_score":7.0,"tags":[]}
    ]"#;

    #[test]
    fn import_orders_outputs_and_builds_events() {
        let csv = "work_id,rating,status,drop_position,total_episodes,rewatches\n\
                   2,4,dropped,3,12,0\n\
                   1,9,completed,,,2\n";
        let dataset = OfflineDataset::import(csv.as_bytes(), CATALOG.as_bytes()).unwrap();

        assert_eq!(dataset.ratings()[0].work_id().get(), 1);
        assert_eq!(dataset.catalog()[0].id().get(), 1);
        assert_eq!(
            dataset.summary(),
            ImportSummary {
                catalog: 2,
                ratings: 2,
                completed: 1,
                dropped: 1,
                rewatches: 2,
            }
        );
    }

    #[test]
    fn catalog_rejects_duplicate_ids() {
        let catalog = r#"[
            {"id":1,"title":"One","global_score":8.0,"tags":[]},
            {"id":1,"title":"Again","global_score":7.0,"tags":[]}
        ]"#;
        let csv = RATINGS_HEADERS.join(",") + "\n";
        let error = OfflineDataset::import(csv.as_bytes(), catalog.as_bytes()).unwrap_err();
        assert_eq!(
            error,
            ImportError::DuplicateCatalogWork {
                work_id: WorkId::new(1).unwrap()
            }
        );
    }
}
