use std::{
    cmp::Ordering,
    collections::HashSet,
    error::Error,
    fmt::{self, Write as _},
};

use serde::{Deserialize, Serialize};

use crate::{
    DatasetError, NormalizedWork, OfflineDataset, ProfileError, RecommendationEngine, ScoringError,
    TasteProfileConfig, WorkId, build_taste_profile,
};

const RANDOM_SEED: u64 = 42;
const RELEVANT_RATING_THRESHOLD: f64 = 8.0;

/// Identifiant stable d'une baseline incluse dans le rapport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineKind {
    WatchMind,
    Random,
    AnilistGlobalScore,
    TagOverlap,
}

impl fmt::Display for BaselineKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(match self {
            Self::WatchMind => "watchmind",
            Self::Random => "random",
            Self::AnilistGlobalScore => "anilist_global_score",
            Self::TagOverlap => "tag_overlap",
        })
    }
}

/// Rang observé pour une œuvre cible masquée pendant son évaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TargetRank {
    work_id: WorkId,
    rank: u32,
}

impl TargetRank {
    #[must_use]
    pub const fn work_id(self) -> WorkId {
        self.work_id
    }

    /// Rang indexé à partir de 1.
    #[must_use]
    pub const fn rank(self) -> u32 {
        self.rank
    }
}

/// Métriques agrégées sur les cibles du harness.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EvaluationMetrics {
    median_rank: f64,
    recall_at_10: f64,
    recall_at_20: f64,
    mean_reciprocal_rank: f64,
}

impl EvaluationMetrics {
    #[must_use]
    pub const fn median_rank(self) -> f64 {
        self.median_rank
    }

    #[must_use]
    pub const fn recall_at_10(self) -> f64 {
        self.recall_at_10
    }

    #[must_use]
    pub const fn recall_at_20(self) -> f64 {
        self.recall_at_20
    }

    #[must_use]
    pub const fn mean_reciprocal_rank(self) -> f64 {
        self.mean_reciprocal_rank
    }
}

/// Résultat détaillé d'une baseline, trié par identifiant de cible.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BaselineResult {
    name: BaselineKind,
    metrics: EvaluationMetrics,
    target_ranks: Vec<TargetRank>,
}

impl BaselineResult {
    #[must_use]
    pub const fn name(&self) -> BaselineKind {
        self.name
    }

    #[must_use]
    pub const fn metrics(&self) -> EvaluationMetrics {
        self.metrics
    }

    #[must_use]
    pub fn target_ranks(&self) -> &[TargetRank] {
        &self.target_ranks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct EvaluationConfiguration {
    random_seed: u64,
    relevant_rating_threshold: f64,
}

/// Rapport déterministe des trois baselines de référence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvaluationReport {
    configuration: EvaluationConfiguration,
    cases: usize,
    baselines: Vec<BaselineResult>,
}

impl EvaluationReport {
    #[must_use]
    pub const fn cases(&self) -> usize {
        self.cases
    }

    #[must_use]
    pub fn baselines(&self) -> &[BaselineResult] {
        &self.baselines
    }

    /// Sérialise le contrat de rapport dans un JSON lisible et stable.
    ///
    /// # Errors
    ///
    /// Propage une éventuelle erreur du sérialiseur JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl fmt::Display for EvaluationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "baseline evaluation: cases={} relevant_rating>={:.1} random_seed={}",
            self.cases,
            self.configuration.relevant_rating_threshold,
            self.configuration.random_seed
        )?;
        writeln!(
            formatter,
            "baseline               median_rank recall@10 recall@20 mrr"
        )?;
        for result in &self.baselines {
            writeln!(
                formatter,
                "{:<22} {:>11.3} {:>9.3} {:>9.3} {:>5.3}",
                result.name,
                result.metrics.median_rank,
                result.metrics.recall_at_10,
                result.metrics.recall_at_20,
                result.metrics.mean_reciprocal_rank
            )?;
        }
        Ok(())
    }
}

/// Erreur empêchant la construction de cas d'évaluation utiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationError {
    NoRelevantRatings,
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRelevantRatings => write!(
                formatter,
                "baseline evaluation requires at least one rating greater than or equal to {RELEVANT_RATING_THRESHOLD:.1}"
            ),
        }
    }
}

impl Error for EvaluationError {}

/// Évalue les trois baselines sur les favoris du dataset par masquage successif.
///
/// Une note supérieure ou égale à 8 est considérée pertinente. Pour chaque
/// cible pertinente, le harness masque cette œuvre, conserve les autres favoris
/// comme historique et classe la cible parmi les œuvres non notées. Les œuvres
/// déjà notées restantes ne peuvent donc pas polluer les candidats.
///
/// # Errors
///
/// Retourne [`EvaluationError::NoRelevantRatings`] si aucune note ne peut servir
/// de cible.
pub fn evaluate_baselines(dataset: &OfflineDataset) -> Result<EvaluationReport, EvaluationError> {
    evaluate_baselines_with(dataset, RELEVANT_RATING_THRESHOLD, RANDOM_SEED)
}

fn evaluate_baselines_with(
    dataset: &OfflineDataset,
    relevant_rating_threshold: f64,
    random_seed: u64,
) -> Result<EvaluationReport, EvaluationError> {
    let cases = evaluation_cases(dataset, relevant_rating_threshold);
    if cases.is_empty() {
        return Err(EvaluationError::NoRelevantRatings);
    }

    let rankers: [&dyn Ranker; 3] = [
        &RandomRanker { seed: random_seed },
        &GlobalScoreRanker,
        &TagOverlapRanker,
    ];
    let baselines = rankers
        .into_iter()
        .map(|ranker| evaluate_ranker(ranker, &cases))
        .collect();

    Ok(EvaluationReport {
        configuration: EvaluationConfiguration {
            random_seed,
            relevant_rating_threshold,
        },
        cases: cases.len(),
        baselines,
    })
}

struct EvaluationCase<'a> {
    target: WorkId,
    liked_history: Vec<&'a NormalizedWork>,
    candidates: Vec<&'a NormalizedWork>,
}

fn evaluation_cases(
    dataset: &OfflineDataset,
    relevant_rating_threshold: f64,
) -> Vec<EvaluationCase<'_>> {
    let rated_ids = dataset
        .ratings()
        .iter()
        .map(crate::RatingRecord::work_id)
        .collect::<HashSet<_>>();
    let relevant_ids = dataset
        .ratings()
        .iter()
        .filter(|rating| rating.rating().get() >= relevant_rating_threshold)
        .map(crate::RatingRecord::work_id)
        .collect::<Vec<_>>();

    relevant_ids
        .iter()
        .map(|target| EvaluationCase {
            target: *target,
            liked_history: dataset
                .catalog()
                .iter()
                .filter(|work| relevant_ids.contains(&work.id()) && work.id() != *target)
                .collect(),
            candidates: dataset
                .catalog()
                .iter()
                .filter(|work| !rated_ids.contains(&work.id()) || work.id() == *target)
                .collect(),
        })
        .collect()
}

trait Ranker {
    fn kind(&self) -> BaselineKind;
    fn rank(&self, case: &EvaluationCase<'_>) -> Vec<WorkId>;
}

struct RandomRanker {
    seed: u64,
}

impl Ranker for RandomRanker {
    fn kind(&self) -> BaselineKind {
        BaselineKind::Random
    }

    fn rank(&self, case: &EvaluationCase<'_>) -> Vec<WorkId> {
        rank_by(case, |work| RandomKey(random_key(self.seed, work.id())))
    }
}

struct GlobalScoreRanker;

impl Ranker for GlobalScoreRanker {
    fn kind(&self) -> BaselineKind {
        BaselineKind::AnilistGlobalScore
    }

    fn rank(&self, case: &EvaluationCase<'_>) -> Vec<WorkId> {
        rank_by(case, |work| {
            DescendingScore(work.global_score().map(crate::Rating::get))
        })
    }
}

struct TagOverlapRanker;

impl Ranker for TagOverlapRanker {
    fn kind(&self) -> BaselineKind {
        BaselineKind::TagOverlap
    }

    fn rank(&self, case: &EvaluationCase<'_>) -> Vec<WorkId> {
        rank_by(case, |candidate| {
            let overlap = candidate
                .tags()
                .iter()
                .map(|candidate_tag| {
                    case.liked_history
                        .iter()
                        .filter_map(|liked| liked.tag_weight(candidate_tag.name()))
                        .map(|liked_weight| liked_weight.get().min(candidate_tag.weight().get()))
                        .fold(0.0, f64::max)
                })
                .sum();
            DescendingScore(Some(overlap))
        })
    }
}

fn rank_by<K: Ord>(case: &EvaluationCase<'_>, key: impl Fn(&NormalizedWork) -> K) -> Vec<WorkId> {
    let mut ranked = case.candidates.clone();
    ranked.sort_by(|left, right| {
        key(left)
            .cmp(&key(right))
            .then_with(|| left.id().cmp(&right.id()))
    });
    ranked.into_iter().map(NormalizedWork::id).collect()
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct RandomKey(u64);

fn random_key(seed: u64, work_id: WorkId) -> u64 {
    let mut value = seed ^ u64::from(work_id.get()).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, PartialEq)]
struct DescendingScore(Option<f64>);

impl Eq for DescendingScore {}

impl PartialOrd for DescendingScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DescendingScore {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0, other.0) {
            (Some(left), Some(right)) => right.total_cmp(&left),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
}

fn evaluate_ranker(ranker: &dyn Ranker, cases: &[EvaluationCase<'_>]) -> BaselineResult {
    let target_ranks = cases
        .iter()
        .map(|case| {
            let ranking = ranker.rank(case);
            let rank = ranking
                .iter()
                .position(|work_id| *work_id == case.target)
                .expect("the evaluation target is always one of the candidates");
            let rank = u32::try_from(rank + 1)
                .expect("a catalog cannot contain more unique works than u32 identifiers");
            TargetRank {
                work_id: case.target,
                rank,
            }
        })
        .collect::<Vec<_>>();
    let metrics = metrics(&target_ranks);
    BaselineResult {
        name: ranker.kind(),
        metrics,
        target_ranks,
    }
}

fn metrics(target_ranks: &[TargetRank]) -> EvaluationMetrics {
    let count = target_count_as_f64(target_ranks.len());
    let mut ordered_ranks = target_ranks
        .iter()
        .map(|target| target.rank)
        .collect::<Vec<_>>();
    ordered_ranks.sort_unstable();
    let midpoint = ordered_ranks.len() / 2;
    let median_rank = if ordered_ranks.len().is_multiple_of(2) {
        f64::midpoint(
            f64::from(ordered_ranks[midpoint - 1]),
            f64::from(ordered_ranks[midpoint]),
        )
    } else {
        f64::from(ordered_ranks[midpoint])
    };

    EvaluationMetrics {
        median_rank,
        recall_at_10: target_count_as_f64(
            target_ranks
                .iter()
                .filter(|target| target.rank <= 10)
                .count(),
        ) / count,
        recall_at_20: target_count_as_f64(
            target_ranks
                .iter()
                .filter(|target| target.rank <= 20)
                .count(),
        ) / count,
        mean_reciprocal_rank: target_ranks
            .iter()
            .map(|target| 1.0 / f64::from(target.rank))
            .sum::<f64>()
            / count,
    }
}

fn target_count_as_f64(count: usize) -> f64 {
    f64::from(
        u32::try_from(count).expect("the harness cannot contain more targets than u32 identifiers"),
    )
}

/// Seuils qui transforment le rapport complet en verrou automatique.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EvaluationThresholds {
    minimum_recall_at_10_delta_vs_tags: f64,
    minimum_mrr_delta_vs_tags: f64,
}

impl Default for EvaluationThresholds {
    fn default() -> Self {
        Self {
            minimum_recall_at_10_delta_vs_tags: 0.0,
            minimum_mrr_delta_vs_tags: 0.0,
        }
    }
}

/// Paire dont l'ordre relatif constitue un cas de regression personnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegressionPair {
    label: String,
    preferred_work_id: WorkId,
    other_work_id: WorkId,
}

/// Date externe utilisee uniquement pour les decoupages temporels du harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalRating {
    work_id: WorkId,
    rated_on: String,
}

/// Configuration versionnee du rapport moteur V1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FullEvaluationConfig {
    profile_version: String,
    seed: u64,
    relevant_rating_threshold: f64,
    minimum_temporal_history: usize,
    thresholds: EvaluationThresholds,
    regression_pairs: Vec<RegressionPair>,
    temporal_ratings: Vec<TemporalRating>,
}

impl Default for FullEvaluationConfig {
    fn default() -> Self {
        Self {
            profile_version: "taste-profile-v1".to_owned(),
            seed: RANDOM_SEED,
            relevant_rating_threshold: RELEVANT_RATING_THRESHOLD,
            minimum_temporal_history: 1,
            thresholds: EvaluationThresholds::default(),
            regression_pairs: Vec::new(),
            temporal_ratings: Vec::new(),
        }
    }
}

/// Resultat d'une paire de regression, avec les deux scores traces.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionResult {
    label: String,
    preferred_work_id: WorkId,
    preferred_score: f64,
    other_work_id: WorkId,
    other_score: f64,
    passed: bool,
}

/// Backtest chronologique. `available=false` signifie qu'aucune date n'a ete fournie.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemporalBacktest {
    available: bool,
    cases: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    metrics: Option<EvaluationMetrics>,
    target_ranks: Vec<TargetRank>,
}

impl TemporalBacktest {
    #[must_use]
    pub const fn available(&self) -> bool {
        self.available
    }

    #[must_use]
    pub const fn cases(&self) -> usize {
        self.cases
    }

    #[must_use]
    pub const fn metrics(&self) -> Option<EvaluationMetrics> {
        self.metrics
    }

    #[must_use]
    pub fn target_ranks(&self) -> &[TargetRank] {
        &self.target_ranks
    }
}

/// Rapport complet du moteur, directement utilisable comme verrou V1.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FullEvaluationReport {
    configuration: FullEvaluationConfig,
    engine: BaselineResult,
    baselines: EvaluationReport,
    regressions: Vec<RegressionResult>,
    temporal_backtest: TemporalBacktest,
    passed: bool,
    failures: Vec<String>,
}

impl FullEvaluationReport {
    #[must_use]
    pub const fn engine(&self) -> &BaselineResult {
        &self.engine
    }

    #[must_use]
    pub const fn baselines(&self) -> &EvaluationReport {
        &self.baselines
    }

    #[must_use]
    pub fn regressions(&self) -> &[RegressionResult] {
        &self.regressions
    }

    #[must_use]
    pub const fn temporal_backtest(&self) -> &TemporalBacktest {
        &self.temporal_backtest
    }

    #[must_use]
    pub const fn passed(&self) -> bool {
        self.passed
    }

    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// Serialise le rapport complet dans un JSON stable.
    ///
    /// # Errors
    ///
    /// Propage une erreur du serialiseur JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let engine = self.engine.metrics;
        let verdict = if self.passed { "PASS" } else { "FAIL" };
        let mut markdown = format!(
            "# WatchMind evaluation - {verdict}\n\n- Profile version: `{}`\n- Seed: `{}`\n- Leave-one-out cases: `{}`\n\n## Ranking metrics\n\n| Ranker | Median rank | Recall@10 | Recall@20 | MRR |\n|---|---:|---:|---:|---:|\n| watchmind | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            self.configuration.profile_version,
            self.configuration.seed,
            self.engine.target_ranks.len(),
            engine.median_rank,
            engine.recall_at_10,
            engine.recall_at_20,
            engine.mean_reciprocal_rank,
        );
        for baseline in &self.baselines.baselines {
            let baseline_metrics = baseline.metrics;
            writeln!(
                markdown,
                "| {} | {:.3} | {:.3} | {:.3} | {:.3} |",
                baseline.name,
                baseline_metrics.median_rank,
                baseline_metrics.recall_at_10,
                baseline_metrics.recall_at_20,
                baseline_metrics.mean_reciprocal_rank,
            )
            .expect("writing to a String cannot fail");
        }
        markdown.push_str("\n## Regression pairs\n\n");
        if self.regressions.is_empty() {
            markdown.push_str("No regression pair configured.\n");
        } else {
            for regression in &self.regressions {
                let status = if regression.passed { "PASS" } else { "FAIL" };
                writeln!(
                    markdown,
                    "- {status} - {}: {} ({:.6}) > {} ({:.6})",
                    regression.label,
                    regression.preferred_work_id.get(),
                    regression.preferred_score,
                    regression.other_work_id.get(),
                    regression.other_score,
                )
                .expect("writing to a String cannot fail");
            }
        }
        markdown.push_str("\n## Temporal backtest\n\n");
        match self.temporal_backtest.metrics {
            Some(temporal) => writeln!(
                markdown,
                "{} cases, median rank {:.3}, Recall@10 {:.3}, MRR {:.3}.",
                self.temporal_backtest.cases,
                temporal.median_rank,
                temporal.recall_at_10,
                temporal.mean_reciprocal_rank,
            )
            .expect("writing to a String cannot fail"),
            None => markdown.push_str("Not available: no usable dated ratings were configured.\n"),
        }
        if !self.failures.is_empty() {
            markdown.push_str("\n## Failures\n\n");
            for failure in &self.failures {
                writeln!(markdown, "- {failure}").expect("writing to a String cannot fail");
            }
        }
        markdown
    }
}

/// Erreur structurelle qui empeche l'evaluation complete de produire un rapport.
#[derive(Debug)]
pub enum FullEvaluationError {
    InvalidConfiguration { field: &'static str, reason: String },
    NoRelevantRatings,
    UnknownWork { work_id: WorkId },
    Dataset(DatasetError),
    Profile(ProfileError),
    Scoring(ScoringError),
}

impl fmt::Display for FullEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { field, reason } => {
                write!(
                    formatter,
                    "invalid evaluation configuration {field}: {reason}"
                )
            }
            Self::NoRelevantRatings => write!(formatter, "evaluation requires a relevant rating"),
            Self::UnknownWork { work_id } => {
                write!(
                    formatter,
                    "evaluation references unknown work {}",
                    work_id.get()
                )
            }
            Self::Dataset(error) => {
                write!(formatter, "cannot assemble evaluation dataset: {error}")
            }
            Self::Profile(error) => write!(formatter, "cannot build evaluation profile: {error}"),
            Self::Scoring(error) => write!(formatter, "cannot score evaluation case: {error}"),
        }
    }
}

impl Error for FullEvaluationError {}

impl From<DatasetError> for FullEvaluationError {
    fn from(error: DatasetError) -> Self {
        Self::Dataset(error)
    }
}

impl From<ProfileError> for FullEvaluationError {
    fn from(error: ProfileError) -> Self {
        Self::Profile(error)
    }
}

impl From<ScoringError> for FullEvaluationError {
    fn from(error: ScoringError) -> Self {
        Self::Scoring(error)
    }
}

/// Evalue le moteur, ses baselines, ses paires de regression et son backtest temporel.
///
/// # Errors
///
/// Refuse une configuration invalide, une reference inconnue ou un dataset qui
/// ne permet pas de construire les profils requis.
pub fn evaluate_full(
    dataset: &OfflineDataset,
    config: &FullEvaluationConfig,
) -> Result<FullEvaluationReport, FullEvaluationError> {
    validate_full_config(dataset, config)?;
    let relevant_ids = dataset
        .ratings()
        .iter()
        .filter(|rating| rating.rating().get() >= config.relevant_rating_threshold)
        .map(crate::RatingRecord::work_id)
        .collect::<Vec<_>>();
    if relevant_ids.is_empty() {
        return Err(FullEvaluationError::NoRelevantRatings);
    }

    let engine_ranks = relevant_ids
        .iter()
        .map(|target| rank_hidden_target(dataset, *target, None))
        .collect::<Result<Vec<_>, FullEvaluationError>>()?;
    let engine = BaselineResult {
        name: BaselineKind::WatchMind,
        metrics: metrics(&engine_ranks),
        target_ranks: engine_ranks,
    };
    let baselines = evaluate_baselines_with(dataset, config.relevant_rating_threshold, config.seed)
        .map_err(|_| FullEvaluationError::NoRelevantRatings)?;
    let regressions = evaluate_regressions(dataset, config)?;
    let temporal_backtest = evaluate_temporal(dataset, config)?;
    let Some(tag_metrics) = baselines
        .baselines
        .iter()
        .find(|baseline| baseline.name == BaselineKind::TagOverlap)
        .map(|baseline| baseline.metrics)
    else {
        return Err(FullEvaluationError::InvalidConfiguration {
            field: "baselines",
            reason: "tag overlap baseline is missing".to_owned(),
        });
    };
    let mut failures = Vec::new();
    let recall_floor =
        tag_metrics.recall_at_10 + config.thresholds.minimum_recall_at_10_delta_vs_tags;
    if engine.metrics.recall_at_10 + f64::EPSILON < recall_floor {
        failures.push(format!(
            "WatchMind Recall@10 {:.3} is below required {:.3}",
            engine.metrics.recall_at_10, recall_floor
        ));
    }
    let mrr_floor = tag_metrics.mean_reciprocal_rank + config.thresholds.minimum_mrr_delta_vs_tags;
    if engine.metrics.mean_reciprocal_rank + f64::EPSILON < mrr_floor {
        failures.push(format!(
            "WatchMind MRR {:.3} is below required {:.3}",
            engine.metrics.mean_reciprocal_rank, mrr_floor
        ));
    }
    failures.extend(
        regressions
            .iter()
            .filter(|regression| !regression.passed)
            .map(|regression| format!("regression pair failed: {}", regression.label)),
    );

    Ok(FullEvaluationReport {
        configuration: config.clone(),
        engine,
        baselines,
        regressions,
        temporal_backtest,
        passed: failures.is_empty(),
        failures,
    })
}

fn validate_full_config(
    dataset: &OfflineDataset,
    config: &FullEvaluationConfig,
) -> Result<(), FullEvaluationError> {
    if config.profile_version.trim().is_empty() {
        return Err(FullEvaluationError::InvalidConfiguration {
            field: "profile_version",
            reason: "must not be empty".to_owned(),
        });
    }
    if !config.relevant_rating_threshold.is_finite()
        || !(0.0..=10.0).contains(&config.relevant_rating_threshold)
    {
        return Err(FullEvaluationError::InvalidConfiguration {
            field: "relevant_rating_threshold",
            reason: "must be between 0 and 10".to_owned(),
        });
    }
    if config.minimum_temporal_history == 0 {
        return Err(FullEvaluationError::InvalidConfiguration {
            field: "minimum_temporal_history",
            reason: "must be greater than zero".to_owned(),
        });
    }
    for (field, value) in [
        (
            "thresholds.minimum_recall_at_10_delta_vs_tags",
            config.thresholds.minimum_recall_at_10_delta_vs_tags,
        ),
        (
            "thresholds.minimum_mrr_delta_vs_tags",
            config.thresholds.minimum_mrr_delta_vs_tags,
        ),
    ] {
        if !value.is_finite() {
            return Err(FullEvaluationError::InvalidConfiguration {
                field,
                reason: "must be finite".to_owned(),
            });
        }
    }
    let catalog_ids = dataset
        .catalog()
        .iter()
        .map(NormalizedWork::id)
        .collect::<HashSet<_>>();
    for work_id in config
        .regression_pairs
        .iter()
        .flat_map(|pair| [pair.preferred_work_id, pair.other_work_id])
        .chain(config.temporal_ratings.iter().map(|rating| rating.work_id))
    {
        if !catalog_ids.contains(&work_id) {
            return Err(FullEvaluationError::UnknownWork { work_id });
        }
    }
    let mut dated_ids = HashSet::new();
    for rating in &config.temporal_ratings {
        if !dated_ids.insert(rating.work_id) {
            return Err(FullEvaluationError::InvalidConfiguration {
                field: "temporal_ratings",
                reason: format!("duplicate work {}", rating.work_id.get()),
            });
        }
        if !valid_iso_date(&rating.rated_on) {
            return Err(FullEvaluationError::InvalidConfiguration {
                field: "temporal_ratings.rated_on",
                reason: format!("{:?} is not a YYYY-MM-DD date", rating.rated_on),
            });
        }
    }
    Ok(())
}

fn valid_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit());
    if !digits {
        return false;
    }
    let year = value[0..4].parse::<u16>().unwrap_or(0);
    let month = value[5..7].parse::<u8>().unwrap_or(0);
    let day = value[8..10].parse::<u8>().unwrap_or(0);
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=days).contains(&day)
}

fn rank_hidden_target(
    dataset: &OfflineDataset,
    target: WorkId,
    training_ids: Option<&HashSet<WorkId>>,
) -> Result<TargetRank, FullEvaluationError> {
    let ratings = dataset
        .ratings()
        .iter()
        .filter(|rating| {
            rating.work_id() != target
                && training_ids.is_none_or(|ids| ids.contains(&rating.work_id()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let rated_ids = ratings
        .iter()
        .map(crate::RatingRecord::work_id)
        .collect::<HashSet<_>>();
    let events = dataset
        .events()
        .iter()
        .filter(|event| rated_ids.contains(&event.work_id()))
        .cloned()
        .collect::<Vec<_>>();
    let training = OfflineDataset::from_parts(dataset.catalog().to_vec(), ratings, events)?;
    let profile = build_taste_profile(&training, &TasteProfileConfig::default())?;
    let candidates = training
        .catalog()
        .iter()
        .filter(|work| !rated_ids.contains(&work.id()))
        .cloned()
        .collect::<Vec<_>>();
    let ranking = RecommendationEngine::default().score_candidates(&profile, &candidates)?;
    let position = ranking
        .iter()
        .position(|recommendation| recommendation.work_id() == target)
        .ok_or(FullEvaluationError::UnknownWork { work_id: target })?;
    let rank =
        u32::try_from(position + 1).map_err(|_| FullEvaluationError::InvalidConfiguration {
            field: "catalog",
            reason: "contains too many works to rank".to_owned(),
        })?;
    Ok(TargetRank {
        work_id: target,
        rank,
    })
}

fn evaluate_regressions(
    dataset: &OfflineDataset,
    config: &FullEvaluationConfig,
) -> Result<Vec<RegressionResult>, FullEvaluationError> {
    if config.regression_pairs.is_empty() {
        return Ok(Vec::new());
    }
    let profile = build_taste_profile(dataset, &TasteProfileConfig::default())?;
    let works = dataset
        .catalog()
        .iter()
        .map(|work| (work.id(), work.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    config
        .regression_pairs
        .iter()
        .map(|pair| {
            let candidates = [
                works
                    .get(&pair.preferred_work_id)
                    .expect("configuration work IDs were validated")
                    .clone(),
                works
                    .get(&pair.other_work_id)
                    .expect("configuration work IDs were validated")
                    .clone(),
            ];
            let scored = RecommendationEngine::default().score_candidates(&profile, &candidates)?;
            let score_for = |work_id| {
                scored
                    .iter()
                    .find(|recommendation| recommendation.work_id() == work_id)
                    .expect("both regression works were scored")
                    .score()
                    .total()
            };
            let preferred_score = score_for(pair.preferred_work_id);
            let other_score = score_for(pair.other_work_id);
            Ok(RegressionResult {
                label: pair.label.clone(),
                preferred_work_id: pair.preferred_work_id,
                preferred_score,
                other_work_id: pair.other_work_id,
                other_score,
                passed: preferred_score > other_score,
            })
        })
        .collect()
}

fn evaluate_temporal(
    dataset: &OfflineDataset,
    config: &FullEvaluationConfig,
) -> Result<TemporalBacktest, FullEvaluationError> {
    if config.temporal_ratings.is_empty() {
        return Ok(TemporalBacktest {
            available: false,
            cases: 0,
            metrics: None,
            target_ranks: Vec::new(),
        });
    }
    let ratings = dataset
        .ratings()
        .iter()
        .map(|rating| (rating.work_id(), rating))
        .collect::<std::collections::HashMap<_, _>>();
    let mut dated = config.temporal_ratings.iter().collect::<Vec<_>>();
    dated.sort_by(|left, right| {
        left.rated_on
            .cmp(&right.rated_on)
            .then_with(|| left.work_id.cmp(&right.work_id))
    });
    let mut target_ranks = Vec::new();
    for dated_rating in &dated {
        let Some(rating) = ratings.get(&dated_rating.work_id) else {
            continue;
        };
        let training_ids = dated
            .iter()
            .filter(|candidate| candidate.rated_on < dated_rating.rated_on)
            .filter(|candidate| ratings.contains_key(&candidate.work_id))
            .map(|candidate| candidate.work_id)
            .collect::<HashSet<_>>();
        if training_ids.len() >= config.minimum_temporal_history
            && rating.rating().get() >= config.relevant_rating_threshold
        {
            target_ranks.push(rank_hidden_target(
                dataset,
                dated_rating.work_id,
                Some(&training_ids),
            )?);
        }
    }
    let temporal_metrics = (!target_ranks.is_empty()).then(|| metrics(&target_ranks));
    Ok(TemporalBacktest {
        available: temporal_metrics.is_some(),
        cases: target_ranks.len(),
        metrics: temporal_metrics,
        target_ranks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_use_one_based_ranks_and_requested_cutoffs() {
        let metrics = metrics(&[
            TargetRank {
                work_id: WorkId::new(1).unwrap(),
                rank: 1,
            },
            TargetRank {
                work_id: WorkId::new(2).unwrap(),
                rank: 4,
            },
            TargetRank {
                work_id: WorkId::new(3).unwrap(),
                rank: 11,
            },
            TargetRank {
                work_id: WorkId::new(4).unwrap(),
                rank: 21,
            },
        ]);

        assert!((metrics.median_rank() - 7.5).abs() < f64::EPSILON);
        assert!((metrics.recall_at_10() - 0.5).abs() < f64::EPSILON);
        assert!((metrics.recall_at_20() - 0.75).abs() < f64::EPSILON);
        let expected_mrr = (1.0 + 0.25 + 1.0 / 11.0 + 1.0 / 21.0) / 4.0;
        assert!((metrics.mean_reciprocal_rank() - expected_mrr).abs() < f64::EPSILON);
    }

    #[test]
    fn random_key_is_stable_and_depends_on_the_identifier() {
        let first = random_key(RANDOM_SEED, WorkId::new(1).unwrap());
        assert_eq!(first, random_key(RANDOM_SEED, WorkId::new(1).unwrap()));
        assert_ne!(first, random_key(RANDOM_SEED, WorkId::new(2).unwrap()));
    }
}
