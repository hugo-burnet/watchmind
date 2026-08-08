use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{
    CandidateSet, NormalizedWork, Ratio, RecommendationEngine, ScoredRecommendation, ScoringError,
    TastePole, TasteProfile, WorkId,
};

/// Réglages validés de la sélection finale diversifiée.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DiversificationConfigData")]
pub struct DiversificationConfig {
    safe_count: usize,
    exploration_count: usize,
    mmr_relevance_weight: Ratio,
    max_per_franchise: usize,
    max_per_studio: usize,
    max_per_dominant_tag: usize,
    dominant_tags_per_work: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct DiversificationConfigData {
    safe_count: usize,
    exploration_count: usize,
    mmr_relevance_weight: Ratio,
    max_per_franchise: usize,
    max_per_studio: usize,
    max_per_dominant_tag: usize,
    dominant_tags_per_work: usize,
}

impl Default for DiversificationConfigData {
    fn default() -> Self {
        Self {
            safe_count: 8,
            exploration_count: 2,
            mmr_relevance_weight: Ratio::new(0.75).expect("default MMR weight is valid"),
            max_per_franchise: 1,
            max_per_studio: 2,
            max_per_dominant_tag: 3,
            dominant_tags_per_work: 2,
        }
    }
}

impl Default for DiversificationConfig {
    fn default() -> Self {
        Self::try_from(DiversificationConfigData::default())
            .expect("default diversification config is valid")
    }
}

impl TryFrom<DiversificationConfigData> for DiversificationConfig {
    type Error = DiversificationError;

    fn try_from(data: DiversificationConfigData) -> Result<Self, Self::Error> {
        if data.safe_count == 0 && data.exploration_count == 0 {
            return Err(DiversificationError::InvalidConfiguration {
                field: "safe_count,exploration_count",
                reason: "at least one requested count must be greater than zero",
            });
        }
        data.safe_count.checked_add(data.exploration_count).ok_or(
            DiversificationError::InvalidConfiguration {
                field: "safe_count,exploration_count",
                reason: "requested size is too large",
            },
        )?;
        for (field, value) in [
            ("max_per_franchise", data.max_per_franchise),
            ("max_per_studio", data.max_per_studio),
            ("max_per_dominant_tag", data.max_per_dominant_tag),
            ("dominant_tags_per_work", data.dominant_tags_per_work),
        ] {
            if value == 0 {
                return Err(DiversificationError::InvalidConfiguration {
                    field,
                    reason: "must be greater than zero",
                });
            }
        }
        Ok(Self {
            safe_count: data.safe_count,
            exploration_count: data.exploration_count,
            mmr_relevance_weight: data.mmr_relevance_weight,
            max_per_franchise: data.max_per_franchise,
            max_per_studio: data.max_per_studio,
            max_per_dominant_tag: data.max_per_dominant_tag,
            dominant_tags_per_work: data.dominant_tags_per_work,
        })
    }
}

/// Rôle d'une recommandation dans la liste finale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    Safe,
    Exploration,
}

/// Signal déterministe qui justifie un pari d'exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplorationSignal {
    UncertainTags,
    PoleDisagreement,
}

/// Libellé explicite associé à un pari.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExplorationLabel {
    signal: ExplorationSignal,
    strength: Ratio,
    text: String,
}

impl ExplorationLabel {
    #[must_use]
    pub const fn signal(&self) -> ExplorationSignal {
        self.signal
    }

    #[must_use]
    pub const fn strength(&self) -> Ratio {
        self.strength
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Recommandation scorée enrichie de son rôle dans la liste finale.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FinalRecommendation {
    kind: RecommendationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    exploration: Option<ExplorationLabel>,
    recommendation: ScoredRecommendation,
}

impl FinalRecommendation {
    #[must_use]
    pub const fn kind(&self) -> RecommendationKind {
        self.kind
    }

    #[must_use]
    pub const fn exploration(&self) -> Option<&ExplorationLabel> {
        self.exploration.as_ref()
    }

    #[must_use]
    pub const fn scored(&self) -> &ScoredRecommendation {
        &self.recommendation
    }
}

/// Liste finale : recommandations sûres d'abord, paris réservés ensuite.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecommendationList {
    safe_count: usize,
    exploration_count: usize,
    recommendations: Vec<FinalRecommendation>,
}

impl RecommendationList {
    #[must_use]
    pub const fn safe_count(&self) -> usize {
        self.safe_count
    }

    #[must_use]
    pub const fn exploration_count(&self) -> usize {
        self.exploration_count
    }

    #[must_use]
    pub fn recommendations(&self) -> &[FinalRecommendation] {
        &self.recommendations
    }
}

impl RecommendationEngine {
    /// Score et sélectionne une liste finale diversifiée.
    ///
    /// La MMR et les plafonds restent derrière cette interface. Une recherche
    /// de faisabilité empêche un choix glouton précoce de réduire la taille de
    /// la liste lorsqu'une combinaison respectant les plafonds existe.
    ///
    /// # Errors
    ///
    /// Propage une erreur si le scoring d'un candidat échoue.
    pub fn recommend(
        &self,
        profile: &TasteProfile,
        candidates: &CandidateSet,
        config: &DiversificationConfig,
    ) -> Result<RecommendationList, DiversificationError> {
        let scored = self.score_candidates(profile, candidates.works())?;
        let mut scored_by_id = scored
            .into_iter()
            .map(|recommendation| (recommendation.work_id(), recommendation))
            .collect::<HashMap<_, _>>();
        let mut choices = candidates
            .works()
            .iter()
            .map(|work| {
                Ok(SelectionCandidate {
                    work: work.clone(),
                    recommendation: scored_by_id.remove(&work.id()).ok_or(
                        DiversificationError::MissingScoredCandidate { work_id: work.id() },
                    )?,
                    safe_relevance: 0.0,
                    exploration: exploration_label(profile, work),
                })
            })
            .collect::<Result<Vec<_>, DiversificationError>>()?;
        normalize_relevance(&mut choices);

        let desired = config
            .safe_count
            .saturating_add(config.exploration_count)
            .min(choices.len());
        let target = maximum_feasible_size(&choices, config, desired);
        let exploration_target = config.exploration_count.min(target);
        let safe_target = config.safe_count.min(target - exploration_target);

        let mut selected = Vec::with_capacity(target);
        let mut limits = LimitState::default();
        select_kind(
            &choices,
            config,
            RecommendationKind::Exploration,
            exploration_target,
            safe_target,
            &mut selected,
            &mut limits,
        );
        select_kind(
            &choices,
            config,
            RecommendationKind::Safe,
            safe_target,
            0,
            &mut selected,
            &mut limits,
        );

        let exploration_ids = selected
            .iter()
            .take(exploration_target)
            .copied()
            .collect::<HashSet<_>>();
        let safe_ids = selected
            .iter()
            .skip(exploration_target)
            .copied()
            .collect::<HashSet<_>>();
        let mut safe = Vec::with_capacity(safe_target);
        let mut exploration = Vec::with_capacity(exploration_target);
        for choice in choices {
            if safe_ids.contains(&choice.work.id()) {
                safe.push(FinalRecommendation {
                    kind: RecommendationKind::Safe,
                    exploration: None,
                    recommendation: choice.recommendation,
                });
            } else if exploration_ids.contains(&choice.work.id()) {
                exploration.push(FinalRecommendation {
                    kind: RecommendationKind::Exploration,
                    exploration: Some(choice.exploration),
                    recommendation: choice.recommendation,
                });
            }
        }
        sort_by_selection_order(&mut safe, &selected[exploration_target..]);
        sort_by_selection_order(&mut exploration, &selected[..exploration_target]);
        safe.extend(exploration);

        Ok(RecommendationList {
            safe_count: safe_target,
            exploration_count: exploration_target,
            recommendations: safe,
        })
    }
}

fn sort_by_selection_order(recommendations: &mut [FinalRecommendation], selected: &[WorkId]) {
    recommendations.sort_by_key(|recommendation| {
        selected
            .iter()
            .position(|id| *id == recommendation.scored().work_id())
            .expect("output recommendation was selected")
    });
}

#[derive(Debug, Clone)]
struct SelectionCandidate {
    work: NormalizedWork,
    recommendation: ScoredRecommendation,
    safe_relevance: f64,
    exploration: ExplorationLabel,
}

fn normalize_relevance(choices: &mut [SelectionCandidate]) {
    let minimum = choices
        .iter()
        .map(|choice| choice.recommendation.score().total())
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);
    let maximum = choices
        .iter()
        .map(|choice| choice.recommendation.score().total())
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let span = maximum - minimum;
    for choice in choices {
        choice.safe_relevance = if span == 0.0 {
            1.0
        } else {
            (choice.recommendation.score().total() - minimum) / span
        };
    }
}

fn exploration_label(profile: &TasteProfile, work: &NormalizedWork) -> ExplorationLabel {
    let total_weight = work
        .tags()
        .iter()
        .map(|tag| tag.weight().get())
        .sum::<f64>();
    let uncertainty = if total_weight == 0.0 {
        1.0
    } else {
        work.tags()
            .iter()
            .map(|tag| {
                let confidence = profile
                    .tag_affinity(tag.name())
                    .map_or(0.0, |affinity| affinity.confidence().get());
                tag.weight().get() * (1.0 - confidence)
            })
            .sum::<f64>()
            / total_weight
    };
    let disagreement = pole_disagreement(profile, work);
    let (signal, strength, text) = if disagreement > uncertainty {
        (
            ExplorationSignal::PoleDisagreement,
            disagreement,
            format!(
                "Pari : les pôles de goût divergent fortement ({disagreement:.0} %)",
                disagreement = disagreement * 100.0
            ),
        )
    } else {
        (
            ExplorationSignal::UncertainTags,
            uncertainty,
            format!(
                "Pari : les affinités de tags restent incertaines ({uncertainty:.0} %)",
                uncertainty = uncertainty * 100.0
            ),
        )
    };
    ExplorationLabel {
        signal,
        strength: Ratio::new(strength.clamp(0.0, 1.0))
            .expect("exploration strength is finite and bounded"),
        text,
    }
}

fn pole_disagreement(profile: &TasteProfile, work: &NormalizedWork) -> f64 {
    if profile.poles().len() < 2 {
        return 0.0;
    }
    let similarities = profile
        .poles()
        .iter()
        .map(|pole| work_pole_similarity(work, pole))
        .collect::<Vec<_>>();
    let minimum = similarities
        .iter()
        .copied()
        .min_by(f64::total_cmp)
        .expect("at least two similarities exist");
    let maximum = similarities
        .iter()
        .copied()
        .max_by(f64::total_cmp)
        .expect("at least two similarities exist");
    (maximum - minimum).clamp(0.0, 1.0)
}

fn work_pole_similarity(work: &NormalizedWork, pole: &TastePole) -> f64 {
    let work_tags = work
        .tags()
        .iter()
        .map(|tag| (normalized_key(tag.name()), tag.weight().get()))
        .collect::<BTreeMap<_, _>>();
    let pole_tags = pole
        .dominant_tags()
        .iter()
        .map(|tag| (normalized_key(tag.name()), tag.weight()))
        .collect::<BTreeMap<_, _>>();
    cosine_similarity(&work_tags, &pole_tags)
}

fn maximum_feasible_size(
    choices: &[SelectionCandidate],
    config: &DiversificationConfig,
    desired: usize,
) -> usize {
    let available = (0..choices.len()).collect::<Vec<_>>();
    for size in (1..=desired).rev() {
        if can_fill(
            choices,
            config,
            &available,
            &mut LimitState::default(),
            size,
        ) {
            return size;
        }
    }
    0
}

fn select_kind(
    choices: &[SelectionCandidate],
    config: &DiversificationConfig,
    kind: RecommendationKind,
    count: usize,
    later_count: usize,
    selected: &mut Vec<WorkId>,
    limits: &mut LimitState,
) {
    for selected_for_kind in 0..count {
        let needed_after = count - selected_for_kind - 1 + later_count;
        let mut ranked = (0..choices.len())
            .filter(|index| !selected.contains(&choices[*index].work.id()))
            .map(|index| {
                let relevance = match kind {
                    RecommendationKind::Safe => choices[index].safe_relevance,
                    RecommendationKind::Exploration => choices[index].exploration.strength().get(),
                };
                let redundancy = selected
                    .iter()
                    .map(|selected_id| {
                        let selected_choice = choices
                            .iter()
                            .find(|choice| choice.work.id() == *selected_id)
                            .expect("selected identifier comes from choices");
                        work_similarity(&choices[index].work, &selected_choice.work)
                    })
                    .max_by(f64::total_cmp)
                    .unwrap_or(0.0);
                let lambda = config.mmr_relevance_weight.get();
                let mmr = lambda * relevance - (1.0 - lambda) * redundancy;
                (index, mmr, relevance)
            })
            .collect::<Vec<_>>();
        ranked.sort_by(
            |(left_index, left_mmr, left_relevance), (right_index, right_mmr, right_relevance)| {
                right_mmr
                    .total_cmp(left_mmr)
                    .then_with(|| right_relevance.total_cmp(left_relevance))
                    .then_with(|| {
                        choices[*right_index]
                            .recommendation
                            .score()
                            .total()
                            .total_cmp(&choices[*left_index].recommendation.score().total())
                    })
                    .then_with(|| {
                        choices[*left_index]
                            .work
                            .id()
                            .cmp(&choices[*right_index].work.id())
                    })
            },
        );

        let next = ranked.into_iter().find_map(|(index, _, _)| {
            if !limits.allows(&choices[index].work, config) {
                return None;
            }
            limits.add(&choices[index].work, config);
            let available = (0..choices.len())
                .filter(|candidate| {
                    *candidate != index && !selected.contains(&choices[*candidate].work.id())
                })
                .collect::<Vec<_>>();
            let feasible = can_fill(choices, config, &available, limits, needed_after);
            limits.remove(&choices[index].work, config);
            feasible.then_some(index)
        });
        let index = next.expect("target size was proven feasible before selection");
        limits.add(&choices[index].work, config);
        selected.push(choices[index].work.id());
    }
}

fn can_fill(
    choices: &[SelectionCandidate],
    config: &DiversificationConfig,
    available: &[usize],
    limits: &mut LimitState,
    needed: usize,
) -> bool {
    if needed == 0 {
        return true;
    }
    if available.len() < needed {
        return false;
    }
    for (position, index) in available.iter().copied().enumerate() {
        if !limits.allows(&choices[index].work, config) {
            continue;
        }
        limits.add(&choices[index].work, config);
        let feasible = can_fill(
            choices,
            config,
            &available[position + 1..],
            limits,
            needed - 1,
        );
        limits.remove(&choices[index].work, config);
        if feasible {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Default)]
struct LimitState {
    franchises: BTreeMap<String, usize>,
    studios: BTreeMap<String, usize>,
    dominant_tags: BTreeMap<String, usize>,
}

impl LimitState {
    fn allows(&self, work: &NormalizedWork, config: &DiversificationConfig) -> bool {
        work.franchise()
            .is_none_or(|franchise| count(&self.franchises, franchise) < config.max_per_franchise)
            && work
                .studios()
                .iter()
                .all(|studio| count(&self.studios, studio) < config.max_per_studio)
            && dominant_tags(work, config.dominant_tags_per_work)
                .iter()
                .all(|tag| count(&self.dominant_tags, tag) < config.max_per_dominant_tag)
    }

    fn add(&mut self, work: &NormalizedWork, config: &DiversificationConfig) {
        if let Some(franchise) = work.franchise() {
            increment(&mut self.franchises, franchise);
        }
        for studio in work.studios() {
            increment(&mut self.studios, studio);
        }
        for tag in dominant_tags(work, config.dominant_tags_per_work) {
            increment(&mut self.dominant_tags, &tag);
        }
    }

    fn remove(&mut self, work: &NormalizedWork, config: &DiversificationConfig) {
        if let Some(franchise) = work.franchise() {
            decrement(&mut self.franchises, franchise);
        }
        for studio in work.studios() {
            decrement(&mut self.studios, studio);
        }
        for tag in dominant_tags(work, config.dominant_tags_per_work) {
            decrement(&mut self.dominant_tags, &tag);
        }
    }
}

fn normalized_key(value: &str) -> String {
    value.to_lowercase()
}

fn count(counts: &BTreeMap<String, usize>, key: &str) -> usize {
    counts.get(&normalized_key(key)).copied().unwrap_or(0)
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(normalized_key(key)).or_default() += 1;
}

fn decrement(counts: &mut BTreeMap<String, usize>, key: &str) {
    let key = normalized_key(key);
    let value = counts
        .get_mut(&key)
        .expect("removed limit was previously added");
    *value -= 1;
    if *value == 0 {
        counts.remove(&key);
    }
}

fn dominant_tags(work: &NormalizedWork, limit: usize) -> Vec<String> {
    let mut tags = work.tags().iter().collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        right
            .weight()
            .get()
            .total_cmp(&left.weight().get())
            .then_with(|| left.name().cmp(right.name()))
    });
    tags.into_iter()
        .take(limit)
        .map(|tag| tag.name().to_owned())
        .collect()
}

fn work_similarity(left: &NormalizedWork, right: &NormalizedWork) -> f64 {
    let left = left
        .tags()
        .iter()
        .map(|tag| (normalized_key(tag.name()), tag.weight().get()))
        .collect::<BTreeMap<_, _>>();
    let right = right
        .tags()
        .iter()
        .map(|tag| (normalized_key(tag.name()), tag.weight().get()))
        .collect::<BTreeMap<_, _>>();
    cosine_similarity(&left, &right)
}

fn cosine_similarity(left: &BTreeMap<String, f64>, right: &BTreeMap<String, f64>) -> f64 {
    let dot = left
        .iter()
        .map(|(name, value)| value * right.get(name).copied().unwrap_or_default())
        .sum::<f64>();
    let left_norm = left.values().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right
        .values()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

/// Échec de configuration ou de scoring de la sélection finale.
#[derive(Debug, Clone, PartialEq)]
pub enum DiversificationError {
    InvalidConfiguration {
        field: &'static str,
        reason: &'static str,
    },
    MissingScoredCandidate {
        work_id: WorkId,
    },
    Scoring(ScoringError),
}

impl fmt::Display for DiversificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { field, reason } => {
                write!(
                    formatter,
                    "invalid diversification configuration {field}: {reason}"
                )
            }
            Self::MissingScoredCandidate { work_id } => write!(
                formatter,
                "scoring did not return candidate {}",
                work_id.get()
            ),
            Self::Scoring(error) => write!(formatter, "cannot diversify recommendations: {error}"),
        }
    }
}

impl Error for DiversificationError {}

impl From<ScoringError> for DiversificationError {
    fn from(error: ScoringError) -> Self {
        Self::Scoring(error)
    }
}
