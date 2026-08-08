use std::{collections::HashSet, error::Error, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const SCORE_TOTAL_EPSILON: f64 = 1.0e-9;

/// Erreur de validation d'un contrat métier.
#[derive(Debug, Clone, PartialEq)]
pub enum DomainError {
    InvalidValue {
        field: &'static str,
        reason: &'static str,
    },
    EmptyText {
        field: &'static str,
    },
    DuplicateTag(String),
    DuplicateAxis(PersonalAxis),
    InconsistentRecommendationTotal {
        declared: f64,
        computed: f64,
    },
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { field, reason } => write!(formatter, "invalid {field}: {reason}"),
            Self::EmptyText { field } => write!(formatter, "{field} must not be empty"),
            Self::DuplicateTag(tag) => write!(formatter, "duplicate tag: {tag}"),
            Self::DuplicateAxis(axis) => write!(formatter, "duplicate personal axis: {axis:?}"),
            Self::InconsistentRecommendationTotal { declared, computed } => write!(
                formatter,
                "recommendation total {declared} differs from contribution sum {computed}"
            ),
        }
    }
}

impl Error for DomainError {}

/// Identifiant `AniList` strictement positif.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct WorkId(u32);

impl WorkId {
    /// Construit un identifiant depuis sa valeur brute.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si `value` vaut zéro.
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::InvalidValue {
                field: "work_id",
                reason: "must be greater than zero",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for WorkId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Note sur 10, finie et comprise entre 0 et 10 inclus.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Rating(f64);

impl Rating {
    /// Construit une note personnelle ou catalogue.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si `value` n'est pas fini ou sort de `[0, 10]`.
    pub fn new(value: f64) -> Result<Self, DomainError> {
        validate_finite_range("rating", value, 0.0, 10.0).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Rating {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Ratio fini compris entre 0 et 1 inclus.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Ratio(f64);

impl Ratio {
    /// Construit un ratio borné.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si `value` n'est pas fini ou sort de `[0, 1]`.
    pub fn new(value: f64) -> Result<Self, DomainError> {
        validate_finite_range("ratio", value, 0.0, 1.0).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Ratio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Poids fini compris entre 0 et 1 inclus.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Weight(f64);

impl Weight {
    /// Construit un poids borné.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si `value` n'est pas fini ou sort de `[0, 1]`.
    pub fn new(value: f64) -> Result<Self, DomainError> {
        validate_finite_range("weight", value, 0.0, 1.0).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Weight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Valeur signée et finie d'une contribution au score.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ScoreDelta(f64);

impl ScoreDelta {
    /// Construit une contribution signée.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si `value` n'est pas fini.
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if !value.is_finite() {
            return Err(DomainError::InvalidValue {
                field: "score_delta",
                reason: "must be finite",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ScoreDelta {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn validate_finite_range(
    field: &'static str,
    value: f64,
    minimum: f64,
    maximum: f64,
) -> Result<f64, DomainError> {
    if !value.is_finite() {
        return Err(DomainError::InvalidValue {
            field,
            reason: "must be finite",
        });
    }
    if !(minimum..=maximum).contains(&value) {
        return Err(DomainError::InvalidValue {
            field,
            reason: "is outside the allowed range",
        });
    }
    Ok(value)
}

/// Tag `AniList` pondéré. Un poids nul est refusé car il ne porte aucun signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TagWeightData")]
pub struct TagWeight {
    name: String,
    weight: Weight,
}

#[derive(Deserialize)]
struct TagWeightData {
    name: String,
    weight: Weight,
}

impl TagWeight {
    /// Construit un tag pondéré et normalise son nom.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si le nom est vide ou le poids nul.
    pub fn new(name: impl Into<String>, weight: Weight) -> Result<Self, DomainError> {
        let name = name.into();
        let name = normalized_text("tag.name", &name)?;
        if weight.get() == 0.0 {
            return Err(DomainError::InvalidValue {
                field: "tag.weight",
                reason: "must be greater than zero",
            });
        }
        Ok(Self { name, weight })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn weight(&self) -> Weight {
        self.weight
    }
}

impl TryFrom<TagWeightData> for TagWeight {
    type Error = DomainError;

    fn try_from(data: TagWeightData) -> Result<Self, Self::Error> {
        Self::new(data.name, data.weight)
    }
}

/// Représentation catalogue minimale utilisée par le moteur offline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NormalizedWorkData")]
pub struct NormalizedWork {
    id: WorkId,
    title: String,
    global_score: Option<Rating>,
    tags: Vec<TagWeight>,
}

#[derive(Deserialize)]
struct NormalizedWorkData {
    id: WorkId,
    title: String,
    global_score: Option<Rating>,
    tags: Vec<TagWeight>,
}

impl NormalizedWork {
    /// Construit une œuvre et ordonne ses tags de façon déterministe.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si le titre est vide ou si deux tags portent le
    /// même nom sans tenir compte de la casse.
    pub fn new(
        id: WorkId,
        title: impl Into<String>,
        global_score: Option<Rating>,
        mut tags: Vec<TagWeight>,
    ) -> Result<Self, DomainError> {
        let title = title.into();
        let title = normalized_text("work.title", &title)?;
        let mut names = HashSet::with_capacity(tags.len());
        for tag in &tags {
            let normalized_name = tag.name.to_lowercase();
            if !names.insert(normalized_name) {
                return Err(DomainError::DuplicateTag(tag.name.clone()));
            }
        }
        tags.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Self {
            id,
            title,
            global_score,
            tags,
        })
    }

    #[must_use]
    pub const fn id(&self) -> WorkId {
        self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn global_score(&self) -> Option<Rating> {
        self.global_score
    }

    #[must_use]
    pub fn tags(&self) -> &[TagWeight] {
        &self.tags
    }

    #[must_use]
    pub fn tag_weight(&self, name: &str) -> Option<Weight> {
        self.tags
            .iter()
            .find(|tag| tag.name.eq_ignore_ascii_case(name))
            .map(TagWeight::weight)
    }
}

impl TryFrom<NormalizedWorkData> for NormalizedWork {
    type Error = DomainError;

    fn try_from(data: NormalizedWorkData) -> Result<Self, Self::Error> {
        Self::new(data.id, data.title, data.global_score, data.tags)
    }
}

/// Les cinq axes sur lesquels une œuvre peut recevoir un crédit personnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalAxis {
    Story,
    Characters,
    WorldBuilding,
    VisualDirection,
    SoundAndMusic,
}

/// Crédit positif accordé à un axe personnel pour une œuvre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AspectCreditData")]
pub struct AspectCredit {
    axis: PersonalAxis,
    credit: Weight,
}

#[derive(Deserialize)]
struct AspectCreditData {
    axis: PersonalAxis,
    credit: Weight,
}

impl AspectCredit {
    /// Construit un crédit positif pour un axe.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si le crédit est nul.
    pub fn new(axis: PersonalAxis, credit: Weight) -> Result<Self, DomainError> {
        if credit.get() == 0.0 {
            return Err(DomainError::InvalidValue {
                field: "aspect_credit.credit",
                reason: "must be greater than zero",
            });
        }
        Ok(Self { axis, credit })
    }

    #[must_use]
    pub const fn axis(&self) -> PersonalAxis {
        self.axis
    }

    #[must_use]
    pub const fn credit(&self) -> Weight {
        self.credit
    }
}

impl TryFrom<AspectCreditData> for AspectCredit {
    type Error = DomainError;

    fn try_from(data: AspectCreditData) -> Result<Self, Self::Error> {
        Self::new(data.axis, data.credit)
    }
}

/// Note personnelle et crédits d'aspects associés à une œuvre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RatingRecordData")]
pub struct RatingRecord {
    work_id: WorkId,
    rating: Rating,
    aspects: Vec<AspectCredit>,
}

#[derive(Deserialize)]
struct RatingRecordData {
    work_id: WorkId,
    rating: Rating,
    #[serde(default)]
    aspects: Vec<AspectCredit>,
}

impl RatingRecord {
    /// Construit une note accompagnée de ses crédits d'aspects.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si plusieurs crédits ciblent le même axe.
    pub fn new(
        work_id: WorkId,
        rating: Rating,
        mut aspects: Vec<AspectCredit>,
    ) -> Result<Self, DomainError> {
        let mut axes = HashSet::with_capacity(aspects.len());
        for aspect in &aspects {
            if !axes.insert(aspect.axis) {
                return Err(DomainError::DuplicateAxis(aspect.axis));
            }
        }
        aspects.sort_by_key(AspectCredit::axis);
        Ok(Self {
            work_id,
            rating,
            aspects,
        })
    }

    #[must_use]
    pub const fn work_id(&self) -> WorkId {
        self.work_id
    }

    #[must_use]
    pub const fn rating(&self) -> Rating {
        self.rating
    }

    #[must_use]
    pub fn aspects(&self) -> &[AspectCredit] {
        &self.aspects
    }

    #[must_use]
    pub fn credit_for(&self, axis: PersonalAxis) -> Option<Weight> {
        self.aspects
            .iter()
            .find(|aspect| aspect.axis == axis)
            .map(AspectCredit::credit)
    }
}

impl TryFrom<RatingRecordData> for RatingRecord {
    type Error = DomainError;

    fn try_from(data: RatingRecordData) -> Result<Self, Self::Error> {
        Self::new(data.work_id, data.rating, data.aspects)
    }
}

/// Avancement d'un abandon. La position doit être strictement inférieure au total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DropProgressData")]
pub struct DropProgress {
    position: u32,
    total: u32,
}

#[derive(Deserialize)]
struct DropProgressData {
    position: u32,
    total: u32,
}

impl DropProgress {
    /// Construit la position d'un abandon dans une œuvre.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si `total` vaut zéro ou si `position >= total`.
    pub fn new(position: u32, total: u32) -> Result<Self, DomainError> {
        if total == 0 {
            return Err(DomainError::InvalidValue {
                field: "drop_progress.total",
                reason: "must be greater than zero",
            });
        }
        if position >= total {
            return Err(DomainError::InvalidValue {
                field: "drop_progress.position",
                reason: "must be lower than total",
            });
        }
        Ok(Self { position, total })
    }

    #[must_use]
    pub const fn position(self) -> u32 {
        self.position
    }

    #[must_use]
    pub const fn total(self) -> u32 {
        self.total
    }

    #[must_use]
    pub fn ratio(self) -> Ratio {
        Ratio(f64::from(self.position) / f64::from(self.total))
    }
}

impl TryFrom<DropProgressData> for DropProgress {
    type Error = DomainError;

    fn try_from(data: DropProgressData) -> Result<Self, Self::Error> {
        Self::new(data.position, data.total)
    }
}

/// Événement de visionnage dont chaque variante ne porte que les données pertinentes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatchEvent {
    Completed {
        work_id: WorkId,
    },
    Dropped {
        work_id: WorkId,
        progress: DropProgress,
    },
    Rewatched {
        work_id: WorkId,
    },
}

impl WatchEvent {
    #[must_use]
    pub const fn completed(work_id: WorkId) -> Self {
        Self::Completed { work_id }
    }

    #[must_use]
    pub const fn dropped(work_id: WorkId, progress: DropProgress) -> Self {
        Self::Dropped { work_id, progress }
    }

    #[must_use]
    pub const fn rewatched(work_id: WorkId) -> Self {
        Self::Rewatched { work_id }
    }

    #[must_use]
    pub const fn work_id(&self) -> WorkId {
        match self {
            Self::Completed { work_id }
            | Self::Dropped { work_id, .. }
            | Self::Rewatched { work_id } => *work_id,
        }
    }

    #[must_use]
    pub const fn drop_progress(&self) -> Option<DropProgress> {
        match self {
            Self::Dropped { progress, .. } => Some(*progress),
            Self::Completed { .. } | Self::Rewatched { .. } => None,
        }
    }
}

/// Origine calculable d'une contribution au score de recommandation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContributionSource {
    TagAffinity,
    PoleSimilarity,
    AnilistPrior,
    PersonalAxis { axis: PersonalAxis },
    Penalty,
}

/// Contribution atomique et explicable au score final.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContributionData")]
pub struct Contribution {
    source: ContributionSource,
    value: ScoreDelta,
    detail: String,
}

#[derive(Deserialize)]
struct ContributionData {
    source: ContributionSource,
    value: ScoreDelta,
    detail: String,
}

impl Contribution {
    /// Construit une contribution explicable.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si le détail textuel est vide.
    pub fn new(
        source: ContributionSource,
        value: ScoreDelta,
        detail: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let detail = detail.into();
        Ok(Self {
            source,
            value,
            detail: normalized_text("contribution.detail", &detail)?,
        })
    }

    #[must_use]
    pub const fn source(&self) -> ContributionSource {
        self.source
    }

    #[must_use]
    pub const fn value(&self) -> ScoreDelta {
        self.value
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl TryFrom<ContributionData> for Contribution {
    type Error = DomainError;

    fn try_from(data: ContributionData) -> Result<Self, Self::Error> {
        Self::new(data.source, data.value, data.detail)
    }
}

/// Score dont le total est toujours dérivé de ses contributions.
#[derive(Debug, Clone, PartialEq)]
pub struct RecommendationScore {
    total: f64,
    contributions: Vec<Contribution>,
}

impl RecommendationScore {
    /// Construit un score et calcule son total exclusivement depuis les contributions.
    ///
    /// # Errors
    ///
    /// Retourne une erreur si la somme sort de la plage des nombres finis.
    pub fn new(contributions: Vec<Contribution>) -> Result<Self, DomainError> {
        let total = contributions
            .iter()
            .try_fold(0.0, |sum, contribution| {
                let next = sum + contribution.value.get();
                next.is_finite().then_some(next)
            })
            .ok_or(DomainError::InvalidValue {
                field: "recommendation_score.total",
                reason: "contribution sum must be finite",
            })?;
        Ok(Self {
            total,
            contributions,
        })
    }

    #[must_use]
    pub const fn total(&self) -> f64 {
        self.total
    }

    #[must_use]
    pub fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }
}

#[derive(Serialize)]
struct RecommendationScoreRef<'a> {
    total: f64,
    contributions: &'a [Contribution],
}

#[derive(Deserialize)]
struct RecommendationScoreData {
    total: f64,
    contributions: Vec<Contribution>,
}

impl Serialize for RecommendationScore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RecommendationScoreRef {
            total: self.total(),
            contributions: &self.contributions,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RecommendationScore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = RecommendationScoreData::deserialize(deserializer)?;
        if !data.total.is_finite() {
            return Err(de::Error::custom(DomainError::InvalidValue {
                field: "recommendation_score.total",
                reason: "must be finite",
            }));
        }
        let score = Self::new(data.contributions).map_err(de::Error::custom)?;
        let computed = score.total();
        if (data.total - computed).abs() > SCORE_TOTAL_EPSILON {
            return Err(de::Error::custom(
                DomainError::InconsistentRecommendationTotal {
                    declared: data.total,
                    computed,
                },
            ));
        }
        Ok(score)
    }
}

fn normalized_text(field: &'static str, value: &str) -> Result<String, DomainError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(DomainError::EmptyText { field });
    }
    Ok(normalized.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_types_reject_invalid_values() {
        assert!(Rating::new(-0.1).is_err());
        assert!(Rating::new(10.1).is_err());
        assert!(Ratio::new(f64::NAN).is_err());
        assert!(Ratio::new(1.1).is_err());
        assert!(Weight::new(-0.1).is_err());
        assert!(ScoreDelta::new(f64::INFINITY).is_err());
    }

    #[test]
    fn normalized_work_rejects_duplicate_tags_ignoring_case() {
        let tags = vec![
            TagWeight::new("Drama", Weight::new(0.8).unwrap()).unwrap(),
            TagWeight::new("drama", Weight::new(0.4).unwrap()).unwrap(),
        ];
        let error = NormalizedWork::new(WorkId::new(1).unwrap(), "Title", None, tags)
            .expect_err("duplicate tag should fail");
        assert_eq!(error, DomainError::DuplicateTag("drama".to_owned()));
    }

    #[test]
    fn rating_record_rejects_duplicate_axes() {
        let aspects = vec![
            AspectCredit::new(PersonalAxis::Story, Weight::new(0.8).unwrap()).unwrap(),
            AspectCredit::new(PersonalAxis::Story, Weight::new(0.5).unwrap()).unwrap(),
        ];
        let error = RatingRecord::new(WorkId::new(1).unwrap(), Rating::new(8.0).unwrap(), aspects)
            .expect_err("duplicate axis should fail");
        assert_eq!(error, DomainError::DuplicateAxis(PersonalAxis::Story));
    }

    #[test]
    fn drop_progress_rejects_completed_or_impossible_drops() {
        assert!(DropProgress::new(1, 0).is_err());
        assert!(DropProgress::new(12, 12).is_err());
        assert!(DropProgress::new(13, 12).is_err());
        assert!((DropProgress::new(3, 12).unwrap().ratio().get() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn recommendation_total_is_derived_from_contributions() {
        let contributions = vec![
            Contribution::new(
                ContributionSource::TagAffinity,
                ScoreDelta::new(1.5).unwrap(),
                "shared drama tags",
            )
            .unwrap(),
            Contribution::new(
                ContributionSource::Penalty,
                ScoreDelta::new(-0.25).unwrap(),
                "franchise prerequisite",
            )
            .unwrap(),
        ];
        assert!(
            (RecommendationScore::new(contributions).unwrap().total() - 1.25).abs() < f64::EPSILON
        );
    }

    #[test]
    fn recommendation_json_rejects_a_forged_total() {
        let json = r#"{
            "total": 99.0,
            "contributions": [{
                "source": { "kind": "anilist_prior" },
                "value": 0.2,
                "detail": "catalog prior"
            }]
        }"#;
        let error = serde_json::from_str::<RecommendationScore>(json)
            .expect_err("forged total should fail");
        assert!(error.to_string().contains("differs from contribution sum"));
    }
}
