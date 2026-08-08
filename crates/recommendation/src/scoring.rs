use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    Contribution, ContributionSource, DomainError, NormalizedWork, RecommendationScore, ScoreDelta,
    TasteProfile, WorkId,
};

const POSITIVE_REASON_LIMIT: usize = 3;
const RISK_LIMIT: usize = 2;

/// Réglages V1 du score explicable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ScoringConfigData")]
pub struct ScoringConfig {
    #[serde(rename = "tag_affinity_weight")]
    tag_affinity: f64,
    #[serde(rename = "negative_tag_penalty_weight")]
    negative_tag_penalty: f64,
    #[serde(rename = "pole_similarity_weight")]
    pole_similarity: f64,
    #[serde(rename = "anilist_prior_weight")]
    anilist_prior: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ScoringConfigData {
    #[serde(rename = "tag_affinity_weight")]
    tag_affinity: f64,
    #[serde(rename = "negative_tag_penalty_weight")]
    negative_tag_penalty: f64,
    #[serde(rename = "pole_similarity_weight")]
    pole_similarity: f64,
    #[serde(rename = "anilist_prior_weight")]
    anilist_prior: f64,
}

impl Default for ScoringConfigData {
    fn default() -> Self {
        Self {
            tag_affinity: 1.0,
            negative_tag_penalty: 1.0,
            pole_similarity: 0.35,
            anilist_prior: 0.10,
        }
    }
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self::try_from(ScoringConfigData::default()).expect("default scoring config is valid")
    }
}

impl TryFrom<ScoringConfigData> for ScoringConfig {
    type Error = ScoringError;

    fn try_from(data: ScoringConfigData) -> Result<Self, Self::Error> {
        validate_positive("tag_affinity_weight", data.tag_affinity)?;
        validate_positive("negative_tag_penalty_weight", data.negative_tag_penalty)?;
        validate_positive("pole_similarity_weight", data.pole_similarity)?;
        validate_range("anilist_prior_weight", data.anilist_prior, 0.0, 0.25)?;
        Ok(Self {
            tag_affinity: data.tag_affinity,
            negative_tag_penalty: data.negative_tag_penalty,
            pole_similarity: data.pole_similarity,
            anilist_prior: data.anilist_prior,
        })
    }
}

fn validate_positive(field: &'static str, value: f64) -> Result<(), ScoringError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ScoringError::InvalidConfiguration {
            field,
            reason: "must be finite and greater than zero",
        });
    }
    Ok(())
}

fn validate_range(
    field: &'static str,
    value: f64,
    minimum: f64,
    maximum: f64,
) -> Result<(), ScoringError> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(ScoringError::InvalidConfiguration {
            field,
            reason: "is outside the allowed range",
        });
    }
    Ok(())
}

/// Projection stable des trois meilleurs motifs et des deux principaux risques.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreExplanation {
    reasons: Vec<Contribution>,
    risks: Vec<Contribution>,
}

impl ScoreExplanation {
    fn from_score(score: &RecommendationScore) -> Self {
        let mut reasons = score
            .contributions()
            .iter()
            .filter(|contribution| contribution.value().get() > 0.0)
            .cloned()
            .collect::<Vec<_>>();
        reasons.sort_by(|left, right| {
            right
                .value()
                .get()
                .total_cmp(&left.value().get())
                .then_with(|| left.detail().cmp(right.detail()))
        });
        reasons.truncate(POSITIVE_REASON_LIMIT);

        let mut risks = score
            .contributions()
            .iter()
            .filter(|contribution| contribution.value().get() < 0.0)
            .cloned()
            .collect::<Vec<_>>();
        risks.sort_by(|left, right| {
            left.value()
                .get()
                .total_cmp(&right.value().get())
                .then_with(|| left.detail().cmp(right.detail()))
        });
        risks.truncate(RISK_LIMIT);

        Self { reasons, risks }
    }

    #[must_use]
    pub fn reasons(&self) -> &[Contribution] {
        &self.reasons
    }

    #[must_use]
    pub fn risks(&self) -> &[Contribution] {
        &self.risks
    }
}

impl fmt::Display for ScoreExplanation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "Raisons :")?;
        if self.reasons.is_empty() {
            writeln!(formatter, "- Aucun signal positif établi")?;
        } else {
            for reason in &self.reasons {
                writeln!(
                    formatter,
                    "- {} ({:+.4})",
                    reason.detail(),
                    reason.value().get()
                )?;
            }
        }
        writeln!(formatter, "Risques :")?;
        if self.risks.is_empty() {
            write!(formatter, "- Aucun risque appris")
        } else {
            for (index, risk) in self.risks.iter().enumerate() {
                if index + 1 == self.risks.len() {
                    write!(
                        formatter,
                        "- {} ({:+.4})",
                        risk.detail(),
                        risk.value().get()
                    )?;
                } else {
                    writeln!(
                        formatter,
                        "- {} ({:+.4})",
                        risk.detail(),
                        risk.value().get()
                    )?;
                }
            }
            Ok(())
        }
    }
}

/// Résultat classé dont le score et le texte proviennent des mêmes contributions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoredRecommendation {
    work_id: WorkId,
    title: String,
    score: RecommendationScore,
    explanation: ScoreExplanation,
}

impl ScoredRecommendation {
    #[must_use]
    pub const fn work_id(&self) -> WorkId {
        self.work_id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn score(&self) -> &RecommendationScore {
        &self.score
    }

    #[must_use]
    pub const fn explanation(&self) -> &ScoreExplanation {
        &self.explanation
    }
}

/// Façade compacte du moteur offline. La génération et le scoring restent deux étapes distinctes.
#[derive(Debug, Clone, PartialEq)]
pub struct RecommendationEngine {
    scoring: ScoringConfig,
}

impl Default for RecommendationEngine {
    fn default() -> Self {
        Self::new(ScoringConfig::default())
    }
}

impl RecommendationEngine {
    #[must_use]
    pub const fn new(scoring: ScoringConfig) -> Self {
        Self { scoring }
    }

    /// Score puis classe des candidats déjà récupérés.
    ///
    /// Le total de chaque résultat est construit uniquement par
    /// [`RecommendationScore::new`], depuis ses contributions atomiques.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si un calcul intermédiaire n'est pas fini.
    pub fn score_candidates(
        &self,
        profile: &TasteProfile,
        candidates: &[NormalizedWork],
    ) -> Result<Vec<ScoredRecommendation>, ScoringError> {
        let mut scored = candidates
            .iter()
            .map(|work| self.score_work(profile, work))
            .collect::<Result<Vec<_>, _>>()?;
        scored.sort_by(|left, right| {
            right
                .score
                .total()
                .total_cmp(&left.score.total())
                .then_with(|| left.work_id.cmp(&right.work_id))
        });
        Ok(scored)
    }

    fn score_work(
        &self,
        profile: &TasteProfile,
        work: &NormalizedWork,
    ) -> Result<ScoredRecommendation, ScoringError> {
        let mut contributions = Vec::new();
        self.add_tag_contributions(profile, work, &mut contributions)?;
        self.add_pole_contribution(profile, work, &mut contributions)?;
        self.add_anilist_prior(work, &mut contributions)?;
        let score = RecommendationScore::new(contributions)?;
        let explanation = ScoreExplanation::from_score(&score);
        Ok(ScoredRecommendation {
            work_id: work.id(),
            title: work.title().to_owned(),
            score,
            explanation,
        })
    }

    /// Ajoute une contribution par tag connu du profil.
    ///
    /// Le terme reste une **somme** et non une moyenne. Diviser par la masse
    /// des tags de l'œuvre paraît plus propre — un titre richement tagué ne
    /// devrait pas accumuler mécaniquement plus de contributions — mais la
    /// mesure sur un historique réel de 150 notes fait chuter le MRR de 0,282 à
    /// 0,145. Le nombre de tags porte une information réelle : une œuvre
    /// abondamment documentée sur `AniList` l'est parce qu'elle est vue et
    /// commentée. Tant que l'évaluation n'aura pas un catalogue assez large
    /// pour séparer ce signal de la simple notoriété, on garde la somme.
    fn add_tag_contributions(
        &self,
        profile: &TasteProfile,
        work: &NormalizedWork,
        contributions: &mut Vec<Contribution>,
    ) -> Result<(), ScoringError> {
        for tag in work.tags() {
            let Some(affinity) = profile.tag_affinity(tag.name()) else {
                continue;
            };
            let raw = tag.weight().get() * affinity.value() * affinity.confidence().get();
            let (source, value, detail) = if raw < 0.0 {
                (
                    ContributionSource::Penalty,
                    raw * self.scoring.negative_tag_penalty,
                    format!("Risque appris pour le tag {}", tag.name()),
                )
            } else {
                (
                    ContributionSource::TagAffinity,
                    raw * self.scoring.tag_affinity,
                    format!("Affinité apprise pour le tag {}", tag.name()),
                )
            };
            contributions.push(contribution(source, value, detail)?);
        }
        Ok(())
    }

    fn add_pole_contribution(
        &self,
        profile: &TasteProfile,
        work: &NormalizedWork,
        contributions: &mut Vec<Contribution>,
    ) -> Result<(), ScoringError> {
        let best = profile
            .poles()
            .iter()
            .map(|pole| (pole, crate::profile::work_pole_similarity(work, pole)))
            .max_by(|(left_pole, left), (right_pole, right)| {
                left.total_cmp(right)
                    .then_with(|| right_pole.ordinal().cmp(&left_pole.ordinal()))
            });
        if let Some((pole, similarity)) = best {
            contributions.push(contribution(
                ContributionSource::PoleSimilarity,
                similarity * self.scoring.pole_similarity,
                format!("Proximité avec le pôle {}", pole.ordinal() + 1),
            )?);
        }
        Ok(())
    }

    fn add_anilist_prior(
        &self,
        work: &NormalizedWork,
        contributions: &mut Vec<Contribution>,
    ) -> Result<(), ScoringError> {
        if let Some(global_score) = work.global_score() {
            let centered = (global_score.get() - 5.0) / 5.0;
            let qualifier = if centered < 0.0 { "faible" } else { "élevé" };
            contributions.push(contribution(
                ContributionSource::AnilistPrior,
                centered * self.scoring.anilist_prior,
                format!("Prior AniList {qualifier} ({:.1}/10)", global_score.get()),
            )?);
        }
        Ok(())
    }
}

fn contribution(
    source: ContributionSource,
    value: f64,
    detail: String,
) -> Result<Contribution, ScoringError> {
    Ok(Contribution::new(source, ScoreDelta::new(value)?, detail)?)
}

/// Échec de configuration ou de calcul du score.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoringError {
    InvalidConfiguration {
        field: &'static str,
        reason: &'static str,
    },
    Domain(DomainError),
}

impl fmt::Display for ScoringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { field, reason } => {
                write!(formatter, "invalid scoring configuration {field}: {reason}")
            }
            Self::Domain(error) => write!(formatter, "cannot score recommendation: {error}"),
        }
    }
}

impl Error for ScoringError {}

impl From<DomainError> for ScoringError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}
