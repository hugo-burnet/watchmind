//! Cœur indépendant du moteur de recommandation `WatchMind`.
//!
//! Les contrats du domaine et les algorithmes seront ajoutés par les lots
//! suivants. Cette crate ne dépendra ni de l'API, ni de `SQLite`, ni du frontend.

mod affinity;
mod domain;
mod evaluation;
mod import;

pub use affinity::{
    AffinityConfig, AffinityError, AffinityReport, PersonalAffinity, RatingSignalKind,
    calculate_affinities,
};
pub use domain::{
    AspectCredit, Contribution, ContributionSource, DomainError, DropProgress, NormalizedWork,
    PersonalAxis, Rating, RatingRecord, Ratio, RecommendationScore, RuntimeMinutes, ScoreDelta,
    TagWeight, WatchEvent, Weight, WorkId,
};
pub use evaluation::{
    BaselineKind, BaselineResult, EvaluationError, EvaluationMetrics, EvaluationReport, TargetRank,
    evaluate_baselines,
};
pub use import::{ImportError, ImportSummary, OfflineDataset};

/// Nom stable du moteur, partagé avec ses adaptateurs.
#[must_use]
pub const fn engine_name() -> &'static str {
    "WatchMind"
}

#[cfg(test)]
mod tests {
    use super::engine_name;

    #[test]
    fn exposes_the_stable_engine_name() {
        assert_eq!(engine_name(), "WatchMind");
    }
}
