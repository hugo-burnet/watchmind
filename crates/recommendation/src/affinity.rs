use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{OfflineDataset, Rating, RuntimeMinutes, WatchEvent, WorkId};

/// Paramètres du calcul d'affinité personnelle.
///
/// La désérialisation valide les bornes et relations entre paramètres. Les
/// valeurs par défaut constituent la configuration V1 documentée du moteur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AffinityConfigData")]
pub struct AffinityConfig {
    rating_scale: f64,
    rating_scale_shrinkage: f64,
    rating_scale_floor: f64,
    rewatch_weight: f64,
    rewatch_duration_reference_minutes: u32,
    rewatch_duration_factor_min: f64,
    rewatch_duration_factor_max: f64,
    drop_penalty_weight: f64,
    drop_curve_exponent: f64,
    good_rating_threshold: Rating,
    good_but_not_for_me_multiplier: f64,
    neutral_band: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AffinityConfigData {
    rating_scale: f64,
    rating_scale_shrinkage: f64,
    rating_scale_floor: f64,
    rewatch_weight: f64,
    rewatch_duration_reference_minutes: u32,
    rewatch_duration_factor_min: f64,
    rewatch_duration_factor_max: f64,
    drop_penalty_weight: f64,
    drop_curve_exponent: f64,
    good_rating_threshold: Rating,
    good_but_not_for_me_multiplier: f64,
    neutral_band: f64,
}

impl Default for AffinityConfigData {
    fn default() -> Self {
        Self {
            rating_scale: 2.0,
            rating_scale_shrinkage: 10.0,
            rating_scale_floor: 0.5,
            rewatch_weight: 0.4,
            rewatch_duration_reference_minutes: 300,
            rewatch_duration_factor_min: 0.5,
            rewatch_duration_factor_max: 2.0,
            drop_penalty_weight: 1.5,
            drop_curve_exponent: 1.5,
            good_rating_threshold: Rating::new(7.0).expect("default rating is valid"),
            good_but_not_for_me_multiplier: 0.5,
            neutral_band: 0.15,
        }
    }
}

impl Default for AffinityConfig {
    fn default() -> Self {
        Self::try_from(AffinityConfigData::default()).expect("default affinity config is valid")
    }
}

impl TryFrom<AffinityConfigData> for AffinityConfig {
    type Error = AffinityError;

    fn try_from(data: AffinityConfigData) -> Result<Self, Self::Error> {
        validate_positive("rating_scale", data.rating_scale)?;
        validate_non_negative("rating_scale_shrinkage", data.rating_scale_shrinkage)?;
        validate_positive("rating_scale_floor", data.rating_scale_floor)?;
        validate_non_negative("rewatch_weight", data.rewatch_weight)?;
        if data.rewatch_duration_reference_minutes == 0 {
            return Err(AffinityError::InvalidConfiguration {
                field: "rewatch_duration_reference_minutes",
                reason: "must be greater than zero",
            });
        }
        validate_positive(
            "rewatch_duration_factor_min",
            data.rewatch_duration_factor_min,
        )?;
        validate_positive(
            "rewatch_duration_factor_max",
            data.rewatch_duration_factor_max,
        )?;
        if data.rewatch_duration_factor_min > data.rewatch_duration_factor_max {
            return Err(AffinityError::InvalidConfiguration {
                field: "rewatch_duration_factor_min",
                reason: "must not exceed rewatch_duration_factor_max",
            });
        }
        validate_non_negative("drop_penalty_weight", data.drop_penalty_weight)?;
        validate_positive("drop_curve_exponent", data.drop_curve_exponent)?;
        validate_unit_interval(
            "good_but_not_for_me_multiplier",
            data.good_but_not_for_me_multiplier,
        )?;
        validate_non_negative("neutral_band", data.neutral_band)?;

        Ok(Self {
            rating_scale: data.rating_scale,
            rating_scale_shrinkage: data.rating_scale_shrinkage,
            rating_scale_floor: data.rating_scale_floor,
            rewatch_weight: data.rewatch_weight,
            rewatch_duration_reference_minutes: data.rewatch_duration_reference_minutes,
            rewatch_duration_factor_min: data.rewatch_duration_factor_min,
            rewatch_duration_factor_max: data.rewatch_duration_factor_max,
            drop_penalty_weight: data.drop_penalty_weight,
            drop_curve_exponent: data.drop_curve_exponent,
            good_rating_threshold: data.good_rating_threshold,
            good_but_not_for_me_multiplier: data.good_but_not_for_me_multiplier,
            neutral_band: data.neutral_band,
        })
    }
}

/// Traitement appliqué au signal de note avant les signaux de visionnage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RatingSignalKind {
    Positive,
    Neutral,
    Negative,
    GoodButNotForMe,
    /// Œuvre abandonnée sans note : seuls les signaux de visionnage parlent.
    Unrated,
}

/// Affinité calculée pour une œuvre, décomposée en signaux auditables.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PersonalAffinity {
    work_id: WorkId,
    value: f64,
    rating_signal: f64,
    rewatch_bonus: f64,
    drop_penalty: f64,
    rating_signal_kind: RatingSignalKind,
    rewatch_count: u32,
}

impl PersonalAffinity {
    #[must_use]
    pub const fn work_id(&self) -> WorkId {
        self.work_id
    }

    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    #[must_use]
    pub const fn rating_signal(&self) -> f64 {
        self.rating_signal
    }

    #[must_use]
    pub const fn rewatch_bonus(&self) -> f64 {
        self.rewatch_bonus
    }

    #[must_use]
    pub const fn drop_penalty(&self) -> f64 {
        self.drop_penalty
    }

    #[must_use]
    pub const fn rating_signal_kind(&self) -> RatingSignalKind {
        self.rating_signal_kind
    }

    #[must_use]
    pub const fn rewatch_count(&self) -> u32 {
        self.rewatch_count
    }
}

/// Résultat déterministe du calcul sur tout un historique.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AffinityReport {
    personal_mean: Rating,
    rating_scale: f64,
    affinities: Vec<PersonalAffinity>,
}

impl AffinityReport {
    #[must_use]
    pub const fn personal_mean(&self) -> Rating {
        self.personal_mean
    }

    /// Échelle effectivement appliquée pour centrer et réduire les notes.
    #[must_use]
    pub const fn rating_scale(&self) -> f64 {
        self.rating_scale
    }

    #[must_use]
    pub fn affinities(&self) -> &[PersonalAffinity] {
        &self.affinities
    }

    #[must_use]
    pub fn affinity_for(&self, work_id: WorkId) -> Option<&PersonalAffinity> {
        self.affinities
            .iter()
            .find(|affinity| affinity.work_id == work_id)
    }
}

/// Échec explicite du calcul d'affinité.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffinityError {
    NoRatings,
    HistoryTooLarge,
    InvalidComputedMean,
    InvalidComputedAffinity {
        work_id: WorkId,
    },
    InvalidConfiguration {
        field: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for AffinityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRatings => write!(formatter, "cannot calculate affinity without ratings"),
            Self::HistoryTooLarge => write!(formatter, "history exceeds supported size"),
            Self::InvalidComputedMean => write!(formatter, "calculated rating mean is invalid"),
            Self::InvalidComputedAffinity { work_id } => write!(
                formatter,
                "calculated affinity for work {} is not finite",
                work_id.get()
            ),
            Self::InvalidConfiguration { field, reason } => {
                write!(
                    formatter,
                    "invalid affinity configuration {field}: {reason}"
                )
            }
        }
    }
}

impl Error for AffinityError {}

/// Calcule une cible d'affinité centrée pour chaque œuvre du vécu.
///
/// Les résultats suivent l'ordre stable des notes du dataset, puis celui des
/// abandons non notés triés par identifiant. La note est centrée sur la moyenne
/// personnelle et réduite par l'échelle personnelle ; les rewatches ajoutent un
/// bonus logarithmique corrigé par la durée ; un abandon ajoute une pénalité
/// dont l'amplitude décroît avec la progression.
///
/// # Errors
///
/// Retourne [`AffinityError::NoRatings`] si l'historique ne contient aucune
/// note.
pub fn calculate_affinities(
    dataset: &OfflineDataset,
    config: &AffinityConfig,
) -> Result<AffinityReport, AffinityError> {
    if dataset.ratings().is_empty() {
        return Err(AffinityError::NoRatings);
    }

    let rating_count =
        u32::try_from(dataset.ratings().len()).map_err(|_| AffinityError::HistoryTooLarge)?;
    let mean = dataset
        .ratings()
        .iter()
        .map(|record| record.rating().get())
        .sum::<f64>()
        / f64::from(rating_count);
    let personal_mean = Rating::new(mean).map_err(|_| AffinityError::InvalidComputedMean)?;
    let rating_scale = personal_rating_scale(dataset, mean, rating_count, config);
    if !rating_scale.is_finite() || rating_scale <= 0.0 {
        return Err(AffinityError::InvalidComputedMean);
    }

    let runtimes = dataset
        .catalog()
        .iter()
        .map(|work| (work.id(), work.runtime_minutes()))
        .collect::<HashMap<_, _>>();
    let event_signals = aggregate_events(dataset.events())?;

    let mut affinities = dataset
        .ratings()
        .iter()
        .map(|record| {
            let work_id = record.work_id();
            let centered = (record.rating().get() - mean) / rating_scale;
            let kind = rating_signal_kind(record.rating(), centered, config);
            let rating_signal = if kind == RatingSignalKind::GoodButNotForMe {
                centered * config.good_but_not_for_me_multiplier
            } else {
                centered
            };
            let events = event_signals.get(&work_id).copied().unwrap_or_default();
            let runtime = runtimes.get(&work_id).copied().flatten();
            personal_affinity(work_id, rating_signal, kind, events, runtime, config)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let rated = dataset
        .ratings()
        .iter()
        .map(crate::RatingRecord::work_id)
        .collect::<HashSet<_>>();
    let mut unrated_drops = event_signals
        .iter()
        .filter(|(work_id, signals)| {
            signals.drop_progress.is_some()
                && !rated.contains(*work_id)
                && runtimes.contains_key(*work_id)
        })
        .map(|(work_id, signals)| (*work_id, *signals))
        .collect::<Vec<_>>();
    unrated_drops.sort_by_key(|(work_id, _)| *work_id);
    for (work_id, events) in unrated_drops {
        let runtime = runtimes.get(&work_id).copied().flatten();
        affinities.push(personal_affinity(
            work_id,
            0.0,
            RatingSignalKind::Unrated,
            events,
            runtime,
            config,
        )?);
    }

    Ok(AffinityReport {
        personal_mean,
        rating_scale,
        affinities,
    })
}

/// Assemble une affinité à partir de son signal de note et de ses événements.
fn personal_affinity(
    work_id: WorkId,
    rating_signal: f64,
    kind: RatingSignalKind,
    events: EventSignals,
    runtime: Option<RuntimeMinutes>,
    config: &AffinityConfig,
) -> Result<PersonalAffinity, AffinityError> {
    let rewatch_bonus = rewatch_bonus(events.rewatch_count, runtime, config);
    let drop_penalty = drop_penalty(events.drop_progress, config);
    let value = rating_signal + rewatch_bonus + drop_penalty;
    if !rating_signal.is_finite()
        || !rewatch_bonus.is_finite()
        || !drop_penalty.is_finite()
        || !value.is_finite()
    {
        return Err(AffinityError::InvalidComputedAffinity { work_id });
    }
    Ok(PersonalAffinity {
        work_id,
        value,
        rating_signal,
        rewatch_bonus,
        drop_penalty,
        rating_signal_kind: kind,
        rewatch_count: events.rewatch_count,
    })
}

/// Échelle de réduction des notes, apprise sur la dispersion de l'utilisateur.
///
/// Une échelle fixe traite de la même façon un noteur qui utilise toute la
/// plage et un noteur qui plafonne entre 7 et 8 : le premier produit des
/// signaux énormes, le second des signaux négligeables. L'écart-type empirique
/// rend le modèle indépendant de l'amplitude de notation, et le shrinkage vers
/// `rating_scale` évite qu'un historique court impose une échelle aberrante.
fn personal_rating_scale(
    dataset: &OfflineDataset,
    mean: f64,
    rating_count: u32,
    config: &AffinityConfig,
) -> f64 {
    let count = f64::from(rating_count);
    let variance = dataset
        .ratings()
        .iter()
        .map(|record| (record.rating().get() - mean).powi(2))
        .sum::<f64>()
        / count;
    let deviation = variance.sqrt();
    let blended = (count * deviation + config.rating_scale_shrinkage * config.rating_scale)
        / (count + config.rating_scale_shrinkage);
    blended.max(config.rating_scale_floor)
}

#[derive(Debug, Clone, Copy, Default)]
struct EventSignals {
    rewatch_count: u32,
    drop_progress: Option<crate::DropProgress>,
}

fn aggregate_events(events: &[WatchEvent]) -> Result<HashMap<WorkId, EventSignals>, AffinityError> {
    let mut signals = HashMap::<WorkId, EventSignals>::new();
    for event in events {
        let entry = signals.entry(event.work_id()).or_default();
        match event {
            WatchEvent::Rewatched { .. } => {
                entry.rewatch_count = entry
                    .rewatch_count
                    .checked_add(1)
                    .ok_or(AffinityError::HistoryTooLarge)?;
            }
            WatchEvent::Dropped { progress, .. } => entry.drop_progress = Some(*progress),
            WatchEvent::Completed { .. } => {}
        }
    }
    Ok(signals)
}

fn rating_signal_kind(rating: Rating, centered: f64, config: &AffinityConfig) -> RatingSignalKind {
    if centered < -config.neutral_band && rating >= config.good_rating_threshold {
        RatingSignalKind::GoodButNotForMe
    } else if centered > config.neutral_band {
        RatingSignalKind::Positive
    } else if centered < -config.neutral_band {
        RatingSignalKind::Negative
    } else {
        RatingSignalKind::Neutral
    }
}

fn rewatch_bonus(count: u32, runtime: Option<RuntimeMinutes>, config: &AffinityConfig) -> f64 {
    let duration_factor = runtime.map_or(1.0, |runtime| {
        (f64::from(runtime.get()) / f64::from(config.rewatch_duration_reference_minutes))
            .sqrt()
            .clamp(
                config.rewatch_duration_factor_min,
                config.rewatch_duration_factor_max,
            )
    });
    config.rewatch_weight * f64::from(count).ln_1p() * duration_factor
}

fn drop_penalty(progress: Option<crate::DropProgress>, config: &AffinityConfig) -> f64 {
    progress.map_or(0.0, |progress| {
        -config.drop_penalty_weight
            * (1.0 - progress.ratio().get()).powf(config.drop_curve_exponent)
    })
}

fn validate_positive(field: &'static str, value: f64) -> Result<(), AffinityError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(AffinityError::InvalidConfiguration {
            field,
            reason: "must be finite and greater than zero",
        });
    }
    Ok(())
}

fn validate_non_negative(field: &'static str, value: f64) -> Result<(), AffinityError> {
    if !value.is_finite() || value < 0.0 {
        return Err(AffinityError::InvalidConfiguration {
            field,
            reason: "must be finite and non-negative",
        });
    }
    Ok(())
}

fn validate_unit_interval(field: &'static str, value: f64) -> Result<(), AffinityError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AffinityError::InvalidConfiguration {
            field,
            reason: "must be finite and between zero and one",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_configuration_round_trips_and_rejects_unknown_fields() {
        let config = AffinityConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            serde_json::from_str::<AffinityConfig>(&json).unwrap(),
            config
        );
        assert!(serde_json::from_str::<AffinityConfig>(r#"{"unknown":1}"#).is_err());
    }

    #[test]
    fn configuration_rejects_invalid_relations_and_non_finite_values() {
        assert!(
            serde_json::from_str::<AffinityConfig>(
                r#"{"rewatch_duration_factor_min":2.0,"rewatch_duration_factor_max":1.0}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<AffinityConfig>(r#"{"drop_curve_exponent":0.0}"#).is_err());
    }
}
