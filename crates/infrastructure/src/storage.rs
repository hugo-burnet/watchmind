use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use watchmind_recommendation::{
    AspectCredit, DomainError, DropProgress, NormalizedWork, OfflineDataset, PersonalAxis, Rating,
    RatingRecord, WatchEvent, Weight, WorkId,
};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite operation failed: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("SQLite migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("stored JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("stored domain value is invalid: {0}")]
    Domain(#[from] DomainError),
    #[error("backup I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("backup format version {0} is unsupported")]
    UnsupportedBackup(u32),
}

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Ouvre/crée une base et applique toutes les migrations embarquées.
    /// # Errors
    /// Retourne une erreur de connexion ou de migration.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Crée une base éphémère partagée, utile aux tests et outils.
    /// # Errors
    /// Retourne une erreur de connexion ou de migration.
    pub async fn in_memory() -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .foreign_keys(true)
            .shared_cache(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    #[must_use]
    pub fn works(&self) -> WorkRepository {
        WorkRepository(self.pool.clone())
    }
    #[must_use]
    pub fn tags(&self) -> TagRepository {
        TagRepository(self.pool.clone())
    }
    #[must_use]
    pub fn ratings(&self) -> RatingRepository {
        RatingRepository(self.pool.clone())
    }
    #[must_use]
    pub fn events(&self) -> EventRepository {
        EventRepository(self.pool.clone())
    }
    #[must_use]
    pub fn aspects(&self) -> AspectRepository {
        AspectRepository(self.pool.clone())
    }
    #[must_use]
    pub fn preferences(&self) -> PreferenceRepository {
        PreferenceRepository(self.pool.clone())
    }
    #[must_use]
    pub fn library(&self) -> LibraryRepository {
        LibraryRepository(self.pool.clone())
    }
    #[must_use]
    pub fn snapshots(&self) -> SnapshotRepository {
        SnapshotRepository(self.pool.clone())
    }

    /// Exporte toutes les données applicatives dans un JSON versionné.
    /// # Errors
    /// Retourne une erreur de lecture SQL, sérialisation ou écriture.
    pub async fn export(&self, path: impl AsRef<Path>) -> Result<(), StorageError> {
        let backup = Backup {
            version: 2,
            works: self.works().all().await?,
            ratings: self.ratings().all().await?,
            events: self.events().all().await?,
            preferences: self.preferences().all().await?,
            library: self.library().all().await?,
            snapshots: self.snapshots().archive().await?,
        };
        tokio::fs::write(path, serde_json::to_vec_pretty(&backup)?).await?;
        Ok(())
    }

    /// Remplace atomiquement le contenu applicatif par celui d'un export.
    /// # Errors
    /// Refuse un export invalide et annule alors toute la transaction.
    pub async fn restore(&self, path: impl AsRef<Path>) -> Result<(), StorageError> {
        let backup: Backup = serde_json::from_slice(&tokio::fs::read(path).await?)?;
        if !matches!(backup.version, 1 | 2) {
            return Err(StorageError::UnsupportedBackup(backup.version));
        }
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM profile_snapshots")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM preferences")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM works")
            .execute(&mut *transaction)
            .await?;
        for work in &backup.works {
            upsert_work(&mut transaction, work).await?;
        }
        for rating in &backup.ratings {
            upsert_rating(&mut transaction, rating).await?;
        }
        for event in &backup.events {
            insert_event(&mut transaction, event).await?;
        }
        for entry in &backup.library {
            sqlx::query("INSERT INTO library(work_id, comment) VALUES (?, ?)")
                .bind(i64::from(entry.work_id.get()))
                .bind(entry.comment.as_deref())
                .execute(&mut *transaction)
                .await?;
        }
        for (key, value) in &backup.preferences {
            sqlx::query("INSERT INTO preferences(key, value) VALUES (?, ?)")
                .bind(key)
                .bind(serde_json::to_string(value)?)
                .execute(&mut *transaction)
                .await?;
        }
        for snapshot in &backup.snapshots {
            insert_snapshot(
                &mut transaction,
                snapshot_timestamp(snapshot.created_at_unix)?,
                &snapshot.profile,
                &snapshot.scores,
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Remplace atomiquement les données personnelles par un dataset offline.
    /// # Errors
    /// Annule toute la transaction si une donnée ne peut pas être persistée.
    pub async fn replace_with_dataset(&self, dataset: &OfflineDataset) -> Result<(), StorageError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM profile_snapshots")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM preferences")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM works")
            .execute(&mut *transaction)
            .await?;
        for work in dataset.catalog() {
            upsert_work(&mut transaction, work).await?;
        }
        for rating in dataset.ratings() {
            upsert_rating(&mut transaction, rating).await?;
        }
        for event in dataset.events() {
            insert_event(&mut transaction, event).await?;
        }
        for work in dataset.catalog() {
            sqlx::query("INSERT INTO library(work_id, comment) VALUES (?, NULL)")
                .bind(i64::from(work.id().get()))
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub work_id: WorkId,
    pub comment: Option<String>,
}

#[derive(Clone)]
pub struct LibraryRepository(SqlitePool);
impl LibraryRepository {
    /// Ajoute l'œuvre à la bibliothèque ou remplace son commentaire.
    /// # Errors
    /// Retourne une erreur SQL, notamment si l'œuvre n'existe pas.
    pub async fn upsert(&self, entry: &LibraryEntry) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO library(work_id, comment) VALUES (?, ?) ON CONFLICT(work_id) DO UPDATE SET comment = excluded.comment")
            .bind(i64::from(entry.work_id.get()))
            .bind(entry.comment.as_deref())
            .execute(&self.0)
            .await?;
        Ok(())
    }

    /// # Errors
    /// Retourne une erreur SQL.
    pub async fn get(&self, id: WorkId) -> Result<Option<LibraryEntry>, StorageError> {
        sqlx::query("SELECT comment FROM library WHERE work_id = ?")
            .bind(i64::from(id.get()))
            .fetch_optional(&self.0)
            .await?
            .map(|row| {
                Ok(LibraryEntry {
                    work_id: id,
                    comment: row.try_get("comment")?,
                })
            })
            .transpose()
    }

    /// Liste les entrées de bibliothèque dans un ordre déterministe.
    /// # Errors
    /// Retourne une erreur SQL ou si un identifiant persisté est invalide.
    pub async fn all(&self) -> Result<Vec<LibraryEntry>, StorageError> {
        sqlx::query("SELECT work_id, comment FROM library ORDER BY work_id")
            .fetch_all(&self.0)
            .await?
            .into_iter()
            .map(|row| {
                let raw_id: i64 = row.try_get("work_id")?;
                let raw_id = u32::try_from(raw_id).map_err(|_| DomainError::InvalidValue {
                    field: "library.work_id",
                    reason: "must fit in an unsigned 32-bit integer",
                })?;
                Ok(LibraryEntry {
                    work_id: WorkId::new(raw_id)?,
                    comment: row.try_get("comment")?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub version: i64,
    pub created_at_unix: u64,
    pub profile: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ArchivedSnapshot {
    created_at_unix: u64,
    profile: serde_json::Value,
    scores: Vec<serde_json::Value>,
}

#[derive(Clone)]
pub struct SnapshotRepository(SqlitePool);
impl SnapshotRepository {
    /// Persiste atomiquement une note, le profil recalculé et tous ses scores.
    /// # Errors
    /// Retourne une erreur sans modifier la note ni créer de snapshot partiel.
    pub async fn create_for_rating(
        &self,
        rating: &RatingRecord,
        created_at_unix: u64,
        profile: &serde_json::Value,
        scores: &[serde_json::Value],
    ) -> Result<i64, StorageError> {
        let timestamp = snapshot_timestamp(created_at_unix)?;
        let mut tx = self.0.begin().await?;
        upsert_rating(&mut tx, rating).await?;
        let version = insert_snapshot(&mut tx, timestamp, profile, scores).await?;
        tx.commit().await?;
        Ok(version)
    }

    /// Retire une œuvre de la bibliothèque et persiste le profil recalculé
    /// dans une transaction unique. L'œuvre catalogue reste disponible afin
    /// de préserver les explications historiques qui la référencent.
    /// # Errors
    /// Retourne une erreur sans suppression ni snapshot partiel.
    pub async fn create_for_removal(
        &self,
        work_id: WorkId,
        created_at_unix: u64,
        profile: &serde_json::Value,
        scores: &[serde_json::Value],
    ) -> Result<i64, StorageError> {
        let timestamp = snapshot_timestamp(created_at_unix)?;
        let mut tx = self.0.begin().await?;
        for table in ["aspects", "events", "ratings", "library"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE work_id = ?"))
                .bind(i64::from(work_id.get()))
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "INSERT INTO preferences(key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(format!("hidden_work:{}", work_id.get()))
        .bind(r#"{"hidden":true}"#)
        .execute(&mut *tx)
        .await?;
        let version = insert_snapshot(&mut tx, timestamp, profile, scores).await?;
        tx.commit().await?;
        Ok(version)
    }

    /// Persiste atomiquement un profil et tous ses scores explicables.
    /// # Errors
    /// Retourne une erreur SQL ou de sérialisation, sans snapshot partiel.
    pub async fn create(
        &self,
        created_at_unix: u64,
        profile: &serde_json::Value,
        scores: &[serde_json::Value],
    ) -> Result<i64, StorageError> {
        let timestamp = snapshot_timestamp(created_at_unix)?;
        let mut tx = self.0.begin().await?;
        let version = insert_snapshot(&mut tx, timestamp, profile, scores).await?;
        tx.commit().await?;
        Ok(version)
    }

    /// # Errors
    /// Retourne une erreur SQL ou si le JSON stocké est invalide.
    pub async fn latest_profile(&self) -> Result<Option<ProfileSnapshot>, StorageError> {
        let row = sqlx::query("SELECT version, created_at_unix, payload FROM profile_snapshots ORDER BY version DESC LIMIT 1")
            .fetch_optional(&self.0).await?;
        row.map(|value| snapshot_from_row(&value)).transpose()
    }

    /// Liste toutes les versions de profil, de la plus récente à la plus ancienne.
    /// # Errors
    /// Retourne une erreur SQL ou si un JSON stocké est invalide.
    pub async fn profiles(&self) -> Result<Vec<ProfileSnapshot>, StorageError> {
        sqlx::query(
            "SELECT version, created_at_unix, payload FROM profile_snapshots ORDER BY version DESC",
        )
        .fetch_all(&self.0)
        .await?
        .into_iter()
        .map(|row| snapshot_from_row(&row))
        .collect()
    }

    /// # Errors
    /// Retourne une erreur SQL ou si le JSON stocké est invalide.
    pub async fn scores(&self, version: i64) -> Result<Vec<serde_json::Value>, StorageError> {
        sqlx::query_scalar::<_, String>(
            "SELECT payload FROM score_snapshots WHERE profile_version = ? ORDER BY rank",
        )
        .bind(version)
        .fetch_all(&self.0)
        .await?
        .into_iter()
        .map(|payload| serde_json::from_str(&payload).map_err(Into::into))
        .collect()
    }

    async fn archive(&self) -> Result<Vec<ArchivedSnapshot>, StorageError> {
        let mut result = Vec::new();
        let mut profiles = self.profiles().await?;
        profiles.reverse();
        for snapshot in profiles {
            result.push(ArchivedSnapshot {
                scores: self.scores(snapshot.version).await?,
                created_at_unix: snapshot.created_at_unix,
                profile: snapshot.profile,
            });
        }
        Ok(result)
    }
}

fn snapshot_timestamp(created_at_unix: u64) -> Result<i64, StorageError> {
    i64::try_from(created_at_unix).map_err(|_| {
        DomainError::InvalidValue {
            field: "created_at_unix",
            reason: "must fit i64",
        }
        .into()
    })
}

async fn insert_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    timestamp: i64,
    profile: &serde_json::Value,
    scores: &[serde_json::Value],
) -> Result<i64, StorageError> {
    let result =
        sqlx::query("INSERT INTO profile_snapshots(created_at_unix, payload) VALUES (?, ?)")
            .bind(timestamp)
            .bind(serde_json::to_string(profile)?)
            .execute(&mut **tx)
            .await?;
    let version = result.last_insert_rowid();
    for (index, score) in scores.iter().enumerate() {
        let work_id = score
            .get("work_id")
            .and_then(serde_json::Value::as_u64)
            .ok_or(DomainError::InvalidValue {
                field: "score.work_id",
                reason: "must be an unsigned integer",
            })?;
        sqlx::query("INSERT INTO score_snapshots(profile_version, work_id, rank, payload) VALUES (?, ?, ?, ?)")
            .bind(version)
            .bind(i64::try_from(work_id).map_err(|_| DomainError::InvalidValue { field: "score.work_id", reason: "must fit i64" })?)
            .bind(i64::try_from(index + 1).map_err(|_| DomainError::InvalidValue { field: "score.rank", reason: "must fit i64" })?)
            .bind(serde_json::to_string(score)?)
            .execute(&mut **tx)
            .await?;
    }
    Ok(version)
}

fn snapshot_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<ProfileSnapshot, StorageError> {
    let timestamp: i64 = row.try_get("created_at_unix")?;
    Ok(ProfileSnapshot {
        version: row.try_get("version")?,
        created_at_unix: u64::try_from(timestamp).map_err(|_| DomainError::InvalidValue {
            field: "created_at_unix",
            reason: "must be non-negative",
        })?,
        profile: serde_json::from_str(&row.try_get::<String, _>("payload")?)?,
    })
}

#[derive(Clone)]
pub struct WorkRepository(SqlitePool);
impl WorkRepository {
    /// # Errors
    /// Retourne une erreur SQL ou de sérialisation.
    pub async fn upsert(&self, work: &NormalizedWork) -> Result<(), StorageError> {
        let mut tx = self.0.begin().await?;
        upsert_work(&mut tx, work).await?;
        tx.commit().await?;
        Ok(())
    }
    /// # Errors
    /// Retourne une erreur SQL ou si les données persistées sont invalides.
    pub async fn get(&self, id: WorkId) -> Result<Option<NormalizedWork>, StorageError> {
        let payload = sqlx::query_scalar::<_, String>("SELECT payload FROM works WHERE id = ?")
            .bind(i64::from(id.get()))
            .fetch_optional(&self.0)
            .await?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
    }
    /// # Errors
    /// Retourne une erreur SQL ou si les données persistées sont invalides.
    pub async fn all(&self) -> Result<Vec<NormalizedWork>, StorageError> {
        sqlx::query_scalar::<_, String>("SELECT payload FROM works ORDER BY id")
            .fetch_all(&self.0)
            .await?
            .into_iter()
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .collect()
    }
}

#[derive(Clone)]
pub struct TagRepository(SqlitePool);
impl TagRepository {
    /// # Errors
    /// Retourne une erreur SQL ou si un poids persisté est invalide.
    pub async fn for_work(&self, id: WorkId) -> Result<Vec<(String, Weight)>, StorageError> {
        let rows = sqlx::query("SELECT name, weight FROM tags WHERE work_id = ? ORDER BY name")
            .bind(i64::from(id.get()))
            .fetch_all(&self.0)
            .await?;
        rows.into_iter()
            .map(|row| Ok((row.try_get("name")?, Weight::new(row.try_get("weight")?)?)))
            .collect()
    }
}

#[derive(Clone)]
pub struct RatingRepository(SqlitePool);
impl RatingRepository {
    /// # Errors
    /// Retourne une erreur SQL ou de validation métier.
    pub async fn upsert(&self, record: &RatingRecord) -> Result<(), StorageError> {
        let mut tx = self.0.begin().await?;
        upsert_rating(&mut tx, record).await?;
        tx.commit().await?;
        Ok(())
    }
    /// # Errors
    /// Retourne une erreur SQL ou si les données persistées sont invalides.
    pub async fn get(&self, id: WorkId) -> Result<Option<RatingRecord>, StorageError> {
        let Some(value) =
            sqlx::query_scalar::<_, f64>("SELECT rating FROM ratings WHERE work_id = ?")
                .bind(i64::from(id.get()))
                .fetch_optional(&self.0)
                .await?
        else {
            return Ok(None);
        };
        Ok(Some(RatingRecord::new(
            id,
            Rating::new(value)?,
            load_aspects(&self.0, id).await?,
        )?))
    }
    async fn all(&self) -> Result<Vec<RatingRecord>, StorageError> {
        let rows = sqlx::query("SELECT work_id, rating FROM ratings ORDER BY work_id")
            .fetch_all(&self.0)
            .await?;
        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let id = WorkId::new(u32::try_from(row.try_get::<i64, _>("work_id")?).map_err(
                |_| DomainError::InvalidValue {
                    field: "work_id",
                    reason: "must fit u32",
                },
            )?)?;
            records.push(RatingRecord::new(
                id,
                Rating::new(row.try_get("rating")?)?,
                load_aspects(&self.0, id).await?,
            )?);
        }
        Ok(records)
    }
}

#[derive(Clone)]
pub struct EventRepository(SqlitePool);
impl EventRepository {
    /// # Errors
    /// Retourne une erreur SQL ou de contrainte d'intégrité.
    pub async fn append(&self, event: &WatchEvent) -> Result<(), StorageError> {
        let mut tx = self.0.begin().await?;
        insert_event(&mut tx, event).await?;
        tx.commit().await?;
        Ok(())
    }
    /// # Errors
    /// Retourne une erreur SQL ou si un événement persisté est invalide.
    pub async fn for_work(&self, id: WorkId) -> Result<Vec<WatchEvent>, StorageError> {
        load_events(
            sqlx::query(
                "SELECT work_id, kind, position, total FROM events WHERE work_id = ? ORDER BY id",
            )
            .bind(i64::from(id.get()))
            .fetch_all(&self.0)
            .await?,
        )
        .collect()
    }
    async fn all(&self) -> Result<Vec<WatchEvent>, StorageError> {
        load_events(
            sqlx::query("SELECT work_id, kind, position, total FROM events ORDER BY id")
                .fetch_all(&self.0)
                .await?,
        )
        .collect()
    }
}

#[derive(Clone)]
pub struct AspectRepository(SqlitePool);
impl AspectRepository {
    /// # Errors
    /// Retourne une erreur SQL ou si un aspect persisté est invalide.
    pub async fn for_work(&self, id: WorkId) -> Result<Vec<AspectCredit>, StorageError> {
        load_aspects(&self.0, id).await
    }
}

#[derive(Clone)]
pub struct PreferenceRepository(SqlitePool);
impl PreferenceRepository {
    /// # Errors
    /// Retourne une erreur SQL ou de sérialisation JSON.
    pub async fn set(&self, key: &str, value: &serde_json::Value) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO preferences(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").bind(key).bind(serde_json::to_string(value)?).execute(&self.0).await?;
        Ok(())
    }
    /// # Errors
    /// Retourne une erreur SQL ou si le JSON persisté est invalide.
    pub async fn get(&self, key: &str) -> Result<Option<serde_json::Value>, StorageError> {
        sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.0)
            .await?
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
    }
    async fn all(&self) -> Result<Vec<(String, serde_json::Value)>, StorageError> {
        sqlx::query("SELECT key, value FROM preferences ORDER BY key")
            .fetch_all(&self.0)
            .await?
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get("key")?,
                    serde_json::from_str(&row.try_get::<String, _>("value")?)?,
                ))
            })
            .collect()
    }
}

async fn upsert_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    work: &NormalizedWork,
) -> Result<(), StorageError> {
    sqlx::query("INSERT INTO works(id, payload) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload").bind(i64::from(work.id().get())).bind(serde_json::to_string(work)?).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM tags WHERE work_id = ?")
        .bind(i64::from(work.id().get()))
        .execute(&mut **tx)
        .await?;
    for tag in work.tags() {
        sqlx::query("INSERT INTO tags(work_id, name, weight) VALUES (?, ?, ?)")
            .bind(i64::from(work.id().get()))
            .bind(tag.name())
            .bind(tag.weight().get())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
async fn upsert_rating(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    record: &RatingRecord,
) -> Result<(), StorageError> {
    sqlx::query("INSERT INTO ratings(work_id, rating) VALUES (?, ?) ON CONFLICT(work_id) DO UPDATE SET rating = excluded.rating").bind(i64::from(record.work_id().get())).bind(record.rating().get()).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM aspects WHERE work_id = ?")
        .bind(i64::from(record.work_id().get()))
        .execute(&mut **tx)
        .await?;
    for aspect in record.aspects() {
        sqlx::query("INSERT INTO aspects(work_id, axis, credit) VALUES (?, ?, ?)")
            .bind(i64::from(record.work_id().get()))
            .bind(axis_name(aspect.axis()))
            .bind(aspect.credit().get())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
async fn insert_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &WatchEvent,
) -> Result<(), StorageError> {
    let (kind, position, total) = match event {
        WatchEvent::Completed { .. } => ("completed", None, None),
        WatchEvent::Rewatched { .. } => ("rewatched", None, None),
        WatchEvent::Dropped { progress, .. } => (
            "dropped",
            Some(i64::from(progress.position())),
            Some(i64::from(progress.total())),
        ),
    };
    sqlx::query("INSERT INTO events(work_id, kind, position, total) VALUES (?, ?, ?, ?)")
        .bind(i64::from(event.work_id().get()))
        .bind(kind)
        .bind(position)
        .bind(total)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn load_aspects(pool: &SqlitePool, id: WorkId) -> Result<Vec<AspectCredit>, StorageError> {
    sqlx::query("SELECT axis, credit FROM aspects WHERE work_id = ? ORDER BY axis")
        .bind(i64::from(id.get()))
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(AspectCredit::new(
                parse_axis(&row.try_get::<String, _>("axis")?)?,
                Weight::new(row.try_get("credit")?)?,
            )?)
        })
        .collect()
}
fn load_events(
    rows: Vec<sqlx::sqlite::SqliteRow>,
) -> impl Iterator<Item = Result<WatchEvent, StorageError>> {
    rows.into_iter().map(|row| {
        let id = WorkId::new(
            u32::try_from(row.try_get::<i64, _>("work_id")?).map_err(|_| {
                DomainError::InvalidValue {
                    field: "work_id",
                    reason: "must fit u32",
                }
            })?,
        )?;
        Ok(match row.try_get::<String, _>("kind")?.as_str() {
            "completed" => WatchEvent::completed(id),
            "rewatched" => WatchEvent::rewatched(id),
            "dropped" => WatchEvent::dropped(
                id,
                DropProgress::new(
                    u32::try_from(row.try_get::<i64, _>("position")?).unwrap_or(0),
                    u32::try_from(row.try_get::<i64, _>("total")?).unwrap_or(0),
                )?,
            ),
            _ => unreachable!("database CHECK constraint"),
        })
    })
}
const fn axis_name(axis: PersonalAxis) -> &'static str {
    match axis {
        PersonalAxis::Story => "story",
        PersonalAxis::Characters => "characters",
        PersonalAxis::WorldBuilding => "world_building",
        PersonalAxis::VisualDirection => "visual_direction",
        PersonalAxis::SoundAndMusic => "sound_and_music",
    }
}
fn parse_axis(value: &str) -> Result<PersonalAxis, DomainError> {
    match value {
        "story" => Ok(PersonalAxis::Story),
        "characters" => Ok(PersonalAxis::Characters),
        "world_building" => Ok(PersonalAxis::WorldBuilding),
        "visual_direction" => Ok(PersonalAxis::VisualDirection),
        "sound_and_music" => Ok(PersonalAxis::SoundAndMusic),
        _ => Err(DomainError::InvalidValue {
            field: "aspect.axis",
            reason: "unknown value",
        }),
    }
}

#[derive(Serialize, Deserialize)]
struct Backup {
    version: u32,
    works: Vec<NormalizedWork>,
    ratings: Vec<RatingRecord>,
    events: Vec<WatchEvent>,
    preferences: Vec<(String, serde_json::Value)>,
    #[serde(default)]
    library: Vec<LibraryEntry>,
    #[serde(default)]
    snapshots: Vec<ArchivedSnapshot>,
}
