use std::{cmp::Ordering, collections::HashSet, error::Error, fmt};

use serde::Serialize;

use crate::{NormalizedWork, OfflineDataset, WorkId};

const RANDOM_SEED: u64 = 42;
const RELEVANT_RATING_THRESHOLD: f64 = 8.0;

/// Identifiant stable d'une baseline incluse dans le rapport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineKind {
    Random,
    AnilistGlobalScore,
    TagOverlap,
}

impl fmt::Display for BaselineKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(match self {
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
    let cases = evaluation_cases(dataset);
    if cases.is_empty() {
        return Err(EvaluationError::NoRelevantRatings);
    }

    let rankers: [&dyn Ranker; 3] = [
        &RandomRanker { seed: RANDOM_SEED },
        &GlobalScoreRanker,
        &TagOverlapRanker,
    ];
    let baselines = rankers
        .into_iter()
        .map(|ranker| evaluate_ranker(ranker, &cases))
        .collect();

    Ok(EvaluationReport {
        configuration: EvaluationConfiguration {
            random_seed: RANDOM_SEED,
            relevant_rating_threshold: RELEVANT_RATING_THRESHOLD,
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

fn evaluation_cases(dataset: &OfflineDataset) -> Vec<EvaluationCase<'_>> {
    let rated_ids = dataset
        .ratings()
        .iter()
        .map(crate::RatingRecord::work_id)
        .collect::<HashSet<_>>();
    let relevant_ids = dataset
        .ratings()
        .iter()
        .filter(|rating| rating.rating().get() >= RELEVANT_RATING_THRESHOLD)
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
