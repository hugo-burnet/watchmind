//! Cœur indépendant du moteur de recommandation `WatchMind`.
//!
//! Les contrats du domaine et les algorithmes seront ajoutés par les lots
//! suivants. Cette crate ne dépendra ni de l'API, ni de `SQLite`, ni du frontend.

mod affinity;
mod candidates;
mod diversification;
mod domain;
mod evaluation;
mod fusion;
mod import;
mod profile;
mod scoring;

pub use affinity::{
    AffinityConfig, AffinityError, AffinityReport, PersonalAffinity, RatingSignalKind,
    calculate_affinities,
};
pub use candidates::{
    CandidateError, CandidateFilter, CandidateReport, CandidateRequest, CandidateSet, RetrievalMode,
};
pub use diversification::{
    DiversificationConfig, DiversificationError, ExplorationLabel, ExplorationSignal,
    FinalRecommendation, RecommendationKind, RecommendationList,
};
pub use domain::{
    AspectCredit, Contribution, ContributionSource, DomainError, DropProgress, NormalizedWork,
    PersonalAxis, Rating, RatingRecord, Ratio, RecommendationScore, ReleaseYear, RuntimeMinutes,
    ScoreDelta, TagWeight, WatchEvent, Weight, WorkFormat, WorkId,
};
pub use evaluation::{
    BaselineKind, BaselineResult, EvaluationError, EvaluationMetrics, EvaluationReport,
    EvaluationThresholds, FullEvaluationConfig, FullEvaluationError, FullEvaluationReport,
    PipelineEvaluation, RegressionPair, RegressionResult, RelevanceMode, TargetRank,
    TemporalBacktest, TemporalRating, evaluate_baselines, evaluate_full,
    evaluate_pipeline_with_request,
};
pub use fusion::rank_candidates_fused;
pub use import::{DatasetError, ImportError, ImportSummary, OfflineDataset};
pub use profile::{
    AxisProfile, AxisWeight, AxisWeightSource, PoleTag, ProfileError, ProfileMode, TagAffinity,
    TastePole, TasteProfile, TasteProfileConfig, build_taste_profile,
};
pub use scoring::{
    RecommendationEngine, ScoreExplanation, ScoredRecommendation, ScoringConfig, ScoringError,
};

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
