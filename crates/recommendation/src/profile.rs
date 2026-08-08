use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{
    AffinityConfig, AffinityError, NormalizedWork, OfflineDataset, PersonalAxis, RatingRecord,
    Ratio, WorkId, calculate_affinities,
};

const MAX_POLES: usize = 4;
const MIN_CLUSTERED_POLES: usize = 2;
const CLUSTER_ITERATIONS: usize = 30;
const AXIS_PRIOR: f64 = 0.2;

/// Configuration complète et sérialisable de la construction du profil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TasteProfileConfigData")]
pub struct TasteProfileConfig {
    affinity: AffinityConfig,
    tag_shrinkage: f64,
    tag_confidence_shrinkage: f64,
    minimum_history_for_clustering: usize,
    favorite_affinity_threshold: f64,
    dominant_tags_per_pole: usize,
    representative_works_per_pole: usize,
    minimum_axis_observations: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TasteProfileConfigData {
    affinity: AffinityConfig,
    tag_shrinkage: f64,
    tag_confidence_shrinkage: f64,
    minimum_history_for_clustering: usize,
    favorite_affinity_threshold: f64,
    dominant_tags_per_pole: usize,
    representative_works_per_pole: usize,
    minimum_axis_observations: usize,
}

impl Default for TasteProfileConfigData {
    fn default() -> Self {
        Self {
            affinity: AffinityConfig::default(),
            tag_shrinkage: 2.0,
            tag_confidence_shrinkage: 150.0,
            minimum_history_for_clustering: 30,
            favorite_affinity_threshold: 0.25,
            dominant_tags_per_pole: 5,
            representative_works_per_pole: 3,
            minimum_axis_observations: 10,
        }
    }
}

impl Default for TasteProfileConfig {
    fn default() -> Self {
        Self::try_from(TasteProfileConfigData::default())
            .expect("default taste profile config is valid")
    }
}

impl TryFrom<TasteProfileConfigData> for TasteProfileConfig {
    type Error = ProfileError;

    fn try_from(data: TasteProfileConfigData) -> Result<Self, Self::Error> {
        validate_positive("tag_shrinkage", data.tag_shrinkage)?;
        validate_finite("tag_confidence_shrinkage", data.tag_confidence_shrinkage)?;
        if data.tag_confidence_shrinkage < 0.0 {
            return Err(ProfileError::InvalidConfiguration {
                field: "tag_confidence_shrinkage",
                reason: "must not be negative",
            });
        }
        validate_positive_count(
            "minimum_history_for_clustering",
            data.minimum_history_for_clustering,
        )?;
        validate_finite(
            "favorite_affinity_threshold",
            data.favorite_affinity_threshold,
        )?;
        if data.favorite_affinity_threshold < 0.0 {
            return Err(ProfileError::InvalidConfiguration {
                field: "favorite_affinity_threshold",
                reason: "must not be negative",
            });
        }
        validate_positive_count("dominant_tags_per_pole", data.dominant_tags_per_pole)?;
        validate_positive_count(
            "representative_works_per_pole",
            data.representative_works_per_pole,
        )?;
        validate_positive_count("minimum_axis_observations", data.minimum_axis_observations)?;
        Ok(Self {
            affinity: data.affinity,
            tag_shrinkage: data.tag_shrinkage,
            tag_confidence_shrinkage: data.tag_confidence_shrinkage,
            minimum_history_for_clustering: data.minimum_history_for_clustering,
            favorite_affinity_threshold: data.favorite_affinity_threshold,
            dominant_tags_per_pole: data.dominant_tags_per_pole,
            representative_works_per_pole: data.representative_works_per_pole,
            minimum_axis_observations: data.minimum_axis_observations,
        })
    }
}

fn validate_positive(field: &'static str, value: f64) -> Result<(), ProfileError> {
    validate_finite(field, value)?;
    if value <= 0.0 {
        return Err(ProfileError::InvalidConfiguration {
            field,
            reason: "must be greater than zero",
        });
    }
    Ok(())
}

fn validate_finite(field: &'static str, value: f64) -> Result<(), ProfileError> {
    if !value.is_finite() {
        return Err(ProfileError::InvalidConfiguration {
            field,
            reason: "must be finite",
        });
    }
    Ok(())
}

fn validate_positive_count(field: &'static str, value: usize) -> Result<(), ProfileError> {
    if value == 0 {
        return Err(ProfileError::InvalidConfiguration {
            field,
            reason: "must be greater than zero",
        });
    }
    Ok(())
}

/// Affinité apprise pour un tag avec ses preuves et sa confiance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TagAffinity {
    name: String,
    /// Clé de correspondance en minuscules, partagée avec le reste du moteur.
    #[serde(skip)]
    key: String,
    value: f64,
    confidence: Ratio,
    evidence_weight: f64,
    observed_works: usize,
}

impl TagAffinity {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    #[must_use]
    pub const fn confidence(&self) -> Ratio {
        self.confidence
    }

    #[must_use]
    pub const fn evidence_weight(&self) -> f64 {
        self.evidence_weight
    }

    #[must_use]
    pub const fn observed_works(&self) -> usize {
        self.observed_works
    }
}

/// Raison explicite du mode à un seul pôle ou du clustering complet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMode {
    SparseHistory,
    SparseFavorites,
    Clustered,
}

/// Poids d'un tag dans la signature d'un pôle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PoleTag {
    name: String,
    weight: f64,
}

impl PoleTag {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }
}

/// Famille cohérente de favoris, résumée par ses tags et œuvres repères.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TastePole {
    ordinal: usize,
    member_count: usize,
    dominant_tags: Vec<PoleTag>,
    representative_work_ids: Vec<WorkId>,
}

impl TastePole {
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    #[must_use]
    pub const fn member_count(&self) -> usize {
        self.member_count
    }

    #[must_use]
    pub fn dominant_tags(&self) -> &[PoleTag] {
        &self.dominant_tags
    }

    #[must_use]
    pub fn representative_work_ids(&self) -> &[WorkId] {
        &self.representative_work_ids
    }
}

/// Origine des poids des axes personnels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisWeightSource {
    Prior,
    Learned,
}

/// Poids normalisé d'un axe personnel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AxisWeight {
    axis: PersonalAxis,
    weight: Ratio,
}

impl AxisWeight {
    #[must_use]
    pub const fn axis(&self) -> PersonalAxis {
        self.axis
    }

    #[must_use]
    pub const fn weight(&self) -> Ratio {
        self.weight
    }
}

/// Distribution des cinq axes avec provenance et volume d'observation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AxisProfile {
    source: AxisWeightSource,
    observed_works: usize,
    weights: Vec<AxisWeight>,
}

impl AxisProfile {
    #[must_use]
    pub const fn source(&self) -> AxisWeightSource {
        self.source
    }

    #[must_use]
    pub const fn observed_works(&self) -> usize {
        self.observed_works
    }

    #[must_use]
    pub fn weights(&self) -> &[AxisWeight] {
        &self.weights
    }

    #[must_use]
    pub fn weight_for(&self, axis: PersonalAxis) -> Ratio {
        self.weights
            .iter()
            .find(|weight| weight.axis == axis)
            .map_or_else(|| ratio(0.0), AxisWeight::weight)
    }
}

/// Profil de goût déterministe prêt à alimenter le scoring.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TasteProfile {
    history_size: usize,
    confidence: Ratio,
    mode: ProfileMode,
    tag_affinities: Vec<TagAffinity>,
    poles: Vec<TastePole>,
    axes: AxisProfile,
}

impl TasteProfile {
    #[must_use]
    pub const fn history_size(&self) -> usize {
        self.history_size
    }

    #[must_use]
    pub const fn confidence(&self) -> Ratio {
        self.confidence
    }

    #[must_use]
    pub const fn mode(&self) -> ProfileMode {
        self.mode
    }

    #[must_use]
    pub fn tag_affinities(&self) -> &[TagAffinity] {
        &self.tag_affinities
    }

    /// Retrouve l'affinité apprise pour un tag, sans tenir compte de la casse.
    ///
    /// Les affinités sont triées par clé normalisée, ce qui permet une
    /// recherche dichotomique. Le scoring interroge cette méthode pour chaque
    /// tag de chaque candidat : un balayage linéaire y coûtait le produit des
    /// deux volumes.
    #[must_use]
    pub fn tag_affinity(&self, name: &str) -> Option<&TagAffinity> {
        let key = normalized_tag_key(name);
        self.tag_affinities
            .binary_search_by(|affinity| affinity.key.as_str().cmp(key.as_str()))
            .ok()
            .map(|index| &self.tag_affinities[index])
    }

    #[must_use]
    pub fn poles(&self) -> &[TastePole] {
        &self.poles
    }

    #[must_use]
    pub const fn axes(&self) -> &AxisProfile {
        &self.axes
    }
}

/// Échec explicite et sans effet de bord de la construction du profil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    Affinity(AffinityError),
    HistoryTooLarge,
    MissingCatalogWork {
        work_id: WorkId,
    },
    InvalidComputedValue {
        field: &'static str,
    },
    InvalidConfiguration {
        field: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Affinity(error) => write!(formatter, "cannot build taste profile: {error}"),
            Self::HistoryTooLarge => write!(formatter, "history exceeds supported size"),
            Self::MissingCatalogWork { work_id } => {
                write!(formatter, "catalog work {} is missing", work_id.get())
            }
            Self::InvalidComputedValue { field } => {
                write!(formatter, "computed taste profile {field} is not finite")
            }
            Self::InvalidConfiguration { field, reason } => {
                write!(
                    formatter,
                    "invalid taste profile configuration {field}: {reason}"
                )
            }
        }
    }
}

impl Error for ProfileError {}

impl From<AffinityError> for ProfileError {
    fn from(error: AffinityError) -> Self {
        Self::Affinity(error)
    }
}

/// Construit toutes les représentations du goût derrière une interface unique.
///
/// Le profil est stable pour un dataset et une configuration identiques. Avec
/// moins de `minimum_history_for_clustering` œuvres, il expose explicitement le
/// mode [`ProfileMode::SparseHistory`] et un unique pôle de repli.
///
/// # Errors
///
/// Propage les erreurs du calcul d'affinité et refuse un dataset incohérent ou
/// trop grand pour ses compteurs déterministes.
pub fn build_taste_profile(
    dataset: &OfflineDataset,
    config: &TasteProfileConfig,
) -> Result<TasteProfile, ProfileError> {
    let affinity_report = calculate_affinities(dataset, &config.affinity)?;
    let history_size = dataset.ratings().len();
    let history_size_u32 =
        u32::try_from(history_size).map_err(|_| ProfileError::HistoryTooLarge)?;
    let works = dataset
        .catalog()
        .iter()
        .map(|work| (work.id(), work))
        .collect::<HashMap<_, _>>();

    let vectors = affinity_report
        .affinities()
        .iter()
        .map(|affinity| {
            let work = works.get(&affinity.work_id()).copied().ok_or(
                ProfileError::MissingCatalogWork {
                    work_id: affinity.work_id(),
                },
            )?;
            Ok(WorkVector::new(work, affinity.value()))
        })
        .collect::<Result<Vec<_>, ProfileError>>()?;

    let tag_affinities = learn_tag_affinities(&vectors, config)?;
    let tagged_works = vectors.iter().filter(|work| !work.tags.is_empty()).count();
    let confidence = profile_confidence(tagged_works, vectors.len(), history_size_u32, config);
    let (mode, poles) = build_poles(&vectors, history_size, config);
    let axes = learn_axis_profile(dataset.ratings(), config);

    Ok(TasteProfile {
        history_size,
        confidence,
        mode,
        tag_affinities,
        poles,
        axes,
    })
}

#[derive(Debug)]
struct TagEvidence {
    display_name: String,
    weighted_target: f64,
    total_weight: f64,
    observed_works: usize,
}

/// Apprend une affinité par tag, pondérée par les preuves accumulées.
///
/// La confiance est le seul facteur de volume : `w / (w + shrinkage)`, qui
/// croît avec les preuves. Elle n'est délibérément pas multipliée par la part
/// du tag dans l'historique, sinon noter des œuvres sans ce tag ferait
/// *baisser* la confiance qu'on lui accorde, et les goûts de niche
/// s'éteindraient à mesure que l'historique grandit.
fn learn_tag_affinities(
    vectors: &[WorkVector],
    config: &TasteProfileConfig,
) -> Result<Vec<TagAffinity>, ProfileError> {
    let mut evidence = BTreeMap::<String, TagEvidence>::new();
    for work in vectors {
        for (name, weight) in &work.tags {
            let entry = evidence
                .entry(normalized_tag_key(name))
                .or_insert_with(|| TagEvidence {
                    display_name: name.clone(),
                    weighted_target: 0.0,
                    total_weight: 0.0,
                    observed_works: 0,
                });
            entry.weighted_target += weight * work.affinity;
            if !entry.weighted_target.is_finite() {
                return Err(ProfileError::InvalidComputedValue {
                    field: "tag_affinity",
                });
            }
            entry.total_weight += weight;
            entry.observed_works += 1;
        }
    }

    Ok(evidence
        .into_iter()
        .map(|(key, evidence)| {
            let value = evidence.weighted_target / (evidence.total_weight + config.tag_shrinkage);
            let volume = evidence.total_weight / (evidence.total_weight + config.tag_shrinkage);
            #[allow(clippy::cast_precision_loss)]
            let observations = evidence.observed_works as f64;
            let breadth = observations / (observations + config.tag_confidence_shrinkage);
            TagAffinity {
                name: evidence.display_name,
                key,
                value,
                confidence: ratio(volume * breadth),
                evidence_weight: evidence.total_weight,
                observed_works: evidence.observed_works,
            }
        })
        .collect())
}

/// Clé de correspondance d'un tag, commune au profil et à la diversification.
pub(crate) fn normalized_tag_key(value: &str) -> String {
    value.to_lowercase()
}

/// Similarité cosinus entre les tags d'une œuvre et la signature d'un pôle.
///
/// Point d'entrée unique du moteur : le scoring, le retrieval et la
/// diversification comparent ainsi une œuvre à un pôle de la même façon, sur
/// des clés normalisées.
pub(crate) fn work_pole_similarity(work: &NormalizedWork, pole: &TastePole) -> f64 {
    let candidate = work
        .tags()
        .iter()
        .map(|tag| (normalized_tag_key(tag.name()), tag.weight().get()))
        .collect::<BTreeMap<_, _>>();
    let signature = pole
        .dominant_tags()
        .iter()
        .map(|tag| (normalized_tag_key(tag.name()), tag.weight()))
        .collect::<BTreeMap<_, _>>();
    cosine_similarity(&candidate, &signature)
}

/// Confiance globale du profil : volume d'historique croisé avec la couverture
/// en tags des œuvres réellement observées.
fn profile_confidence(
    tagged_works: usize,
    observed_works: usize,
    history_size: u32,
    config: &TasteProfileConfig,
) -> Ratio {
    if observed_works == 0 {
        return ratio(0.0);
    }
    let minimum_history = u32::try_from(config.minimum_history_for_clustering).unwrap_or(u32::MAX);
    let volume = f64::from(history_size) / f64::from(minimum_history);
    #[allow(clippy::cast_precision_loss)]
    let coverage = tagged_works as f64 / observed_works as f64;
    ratio(volume.min(1.0) * coverage)
}

#[derive(Debug, Clone)]
struct WorkVector {
    work_id: WorkId,
    affinity: f64,
    tags: BTreeMap<String, f64>,
}

impl WorkVector {
    fn new(work: &NormalizedWork, affinity: f64) -> Self {
        Self {
            work_id: work.id(),
            affinity,
            tags: work
                .tags()
                .iter()
                .map(|tag| (tag.name().to_owned(), tag.weight().get()))
                .collect(),
        }
    }
}

fn build_poles(
    vectors: &[WorkVector],
    history_size: usize,
    config: &TasteProfileConfig,
) -> (ProfileMode, Vec<TastePole>) {
    let mut favorites = vectors
        .iter()
        .filter(|work| work.affinity >= config.favorite_affinity_threshold)
        .cloned()
        .collect::<Vec<_>>();
    favorites.sort_by_key(|work| work.work_id);

    if history_size < config.minimum_history_for_clustering {
        return (
            ProfileMode::SparseHistory,
            vec![fallback_pole(vectors, &favorites, config)],
        );
    }
    if favorites.len() < MIN_CLUSTERED_POLES {
        return (
            ProfileMode::SparseFavorites,
            vec![fallback_pole(vectors, &favorites, config)],
        );
    }

    let desired = history_size
        .checked_div(config.minimum_history_for_clustering)
        .unwrap_or(0)
        .saturating_add(1)
        .clamp(MIN_CLUSTERED_POLES, MAX_POLES)
        .min(favorites.len());
    (
        ProfileMode::Clustered,
        cluster_favorites(&favorites, desired, config),
    )
}

fn fallback_pole(
    vectors: &[WorkVector],
    favorites: &[WorkVector],
    config: &TasteProfileConfig,
) -> TastePole {
    if favorites.is_empty() {
        let best = vectors
            .iter()
            .max_by(|left, right| {
                left.affinity
                    .total_cmp(&right.affinity)
                    .then_with(|| right.work_id.cmp(&left.work_id))
            })
            .expect("affinity calculation rejects an empty history");
        pole_from_members(0, std::slice::from_ref(best), config)
    } else {
        pole_from_members(0, favorites, config)
    }
}

fn cluster_favorites(
    favorites: &[WorkVector],
    pole_count: usize,
    config: &TasteProfileConfig,
) -> Vec<TastePole> {
    let distances = pairwise_distances(favorites);
    let mut centroids = select_seeds(&distances, pole_count)
        .into_iter()
        .map(|seed| favorites[seed].tags.clone())
        .collect::<Vec<_>>();
    let mut assignments = vec![usize::MAX; favorites.len()];

    for _ in 0..CLUSTER_ITERATIONS {
        let next = favorites
            .iter()
            .map(|work| closest_centroid(&work.tags, &centroids))
            .collect::<Vec<_>>();
        let next = repair_empty_clusters(favorites, &centroids, next, pole_count);
        let stable = next == assignments;
        assignments = next;
        centroids = (0..pole_count)
            .map(|cluster| {
                let members = favorites
                    .iter()
                    .zip(&assignments)
                    .filter_map(|(work, assignment)| (*assignment == cluster).then_some(work))
                    .collect::<Vec<_>>();
                centroid(&members)
            })
            .collect();
        if stable {
            break;
        }
    }

    (0..pole_count)
        .map(|cluster| {
            let members = favorites
                .iter()
                .zip(&assignments)
                .filter_map(|(work, assignment)| (*assignment == cluster).then_some(work.clone()))
                .collect::<Vec<_>>();
            pole_from_members(cluster, &members, config)
        })
        .collect()
}

/// Distances cosinus deux à deux, calculées une seule fois par clustering.
fn pairwise_distances(favorites: &[WorkVector]) -> Vec<Vec<f64>> {
    let size = favorites.len();
    let mut distances = vec![vec![0.0; size]; size];
    for (left, work) in favorites.iter().enumerate() {
        for (right, other) in favorites.iter().enumerate().skip(left + 1) {
            let distance = 1.0 - cosine_similarity(&work.tags, &other.tags);
            distances[left][right] = distance;
            distances[right][left] = distance;
        }
    }
    distances
}

/// Choisit les germes par k-means++ glouton et déterministe.
///
/// Le premier germe est le médoïde, donc un point de la zone dense. Chaque
/// germe suivant est celui qui minimise le potentiel résiduel. L'échantillonnage
/// du point le plus lointain, lui, retenait par construction les favoris les
/// plus atypiques et ancrait chaque pôle sur un outlier.
fn select_seeds(distances: &[Vec<f64>], count: usize) -> Vec<usize> {
    let size = distances.len();
    let medoid = (0..size)
        .min_by(|left, right| {
            potential_of(&distances[*left])
                .total_cmp(&potential_of(&distances[*right]))
                .then_with(|| left.cmp(right))
        })
        .expect("clustered favorites are not empty");
    let mut seeds = vec![medoid];
    let mut nearest = distances[medoid]
        .iter()
        .map(|distance| distance * distance)
        .collect::<Vec<_>>();

    while seeds.len() < count {
        let Some(best) = (0..size)
            .filter(|index| !seeds.contains(index))
            .min_by(|left, right| {
                residual_potential(&distances[*left], &nearest)
                    .total_cmp(&residual_potential(&distances[*right], &nearest))
                    .then_with(|| left.cmp(right))
            })
        else {
            break;
        };
        for (index, value) in nearest.iter_mut().enumerate() {
            let alternative = distances[best][index] * distances[best][index];
            if alternative < *value {
                *value = alternative;
            }
        }
        seeds.push(best);
    }
    seeds
}

fn potential_of(distances: &[f64]) -> f64 {
    distances
        .iter()
        .map(|distance| distance * distance)
        .sum::<f64>()
}

fn residual_potential(distances: &[f64], nearest: &[f64]) -> f64 {
    distances
        .iter()
        .zip(nearest)
        .map(|(distance, current)| current.min(distance * distance))
        .sum::<f64>()
}

/// Redonne un membre à chaque cluster vidé par l'affectation.
///
/// L'ancienne implémentation épinglait définitivement chaque germe à son
/// cluster pour garantir des pôles non vides, au prix d'un centroïde qui ne
/// pouvait plus dériver vers la vraie zone dense. On laisse désormais Lloyd
/// travailler librement et on répare le seul effet indésirable : le cluster
/// vide reçoit le point le plus mal servi par son propre centroïde.
fn repair_empty_clusters(
    favorites: &[WorkVector],
    centroids: &[BTreeMap<String, f64>],
    mut assignments: Vec<usize>,
    pole_count: usize,
) -> Vec<usize> {
    let mut sizes = vec![0_usize; pole_count];
    for assignment in &assignments {
        sizes[*assignment] += 1;
    }

    for cluster in 0..pole_count {
        if sizes[cluster] > 0 {
            continue;
        }
        let donor = (0..favorites.len())
            .filter(|index| sizes[assignments[*index]] > 1)
            .min_by(|left, right| {
                cosine_similarity(&favorites[*left].tags, &centroids[assignments[*left]])
                    .total_cmp(&cosine_similarity(
                        &favorites[*right].tags,
                        &centroids[assignments[*right]],
                    ))
                    .then_with(|| favorites[*left].work_id.cmp(&favorites[*right].work_id))
            });
        if let Some(donor) = donor {
            sizes[assignments[donor]] -= 1;
            assignments[donor] = cluster;
            sizes[cluster] += 1;
        }
    }
    assignments
}

fn closest_centroid(tags: &BTreeMap<String, f64>, centroids: &[BTreeMap<String, f64>]) -> usize {
    centroids
        .iter()
        .enumerate()
        .map(|(index, centroid)| (index, cosine_similarity(tags, centroid)))
        .max_by(|(left_index, left), (right_index, right)| {
            left.total_cmp(right)
                .then_with(|| right_index.cmp(left_index))
        })
        .expect("at least two centroids exist")
        .0
}

fn centroid(members: &[&WorkVector]) -> BTreeMap<String, f64> {
    let mut centroid = BTreeMap::<String, f64>::new();
    for work in members {
        for (name, weight) in &work.tags {
            *centroid.entry(name.clone()).or_default() += weight;
        }
    }
    let member_count = u32::try_from(members.len()).expect("members cannot exceed history size");
    for value in centroid.values_mut() {
        *value /= f64::from(member_count);
    }
    centroid
}

fn pole_from_members(
    ordinal: usize,
    members: &[WorkVector],
    config: &TasteProfileConfig,
) -> TastePole {
    let member_refs = members.iter().collect::<Vec<_>>();
    let centroid = centroid(&member_refs);
    let mut dominant_tags = centroid
        .iter()
        .map(|(name, weight)| PoleTag {
            name: name.clone(),
            weight: *weight,
        })
        .collect::<Vec<_>>();
    dominant_tags.sort_by(|left, right| {
        right
            .weight
            .total_cmp(&left.weight)
            .then_with(|| left.name.cmp(&right.name))
    });
    dominant_tags.truncate(config.dominant_tags_per_pole);

    let mut representatives = members.iter().collect::<Vec<_>>();
    representatives.sort_by(|left, right| {
        cosine_similarity(&right.tags, &centroid)
            .total_cmp(&cosine_similarity(&left.tags, &centroid))
            .then_with(|| right.affinity.total_cmp(&left.affinity))
            .then_with(|| left.work_id.cmp(&right.work_id))
    });
    let representative_work_ids = representatives
        .into_iter()
        .take(config.representative_works_per_pole)
        .map(|work| work.work_id)
        .collect();

    TastePole {
        ordinal,
        member_count: members.len(),
        dominant_tags,
        representative_work_ids,
    }
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

fn learn_axis_profile(ratings: &[RatingRecord], config: &TasteProfileConfig) -> AxisProfile {
    let observed_works = ratings
        .iter()
        .filter(|rating| !rating.aspects().is_empty())
        .count();
    if observed_works < config.minimum_axis_observations {
        return prior_axis_profile(observed_works);
    }

    let axes = all_axes();
    let sums = axes
        .iter()
        .map(|axis| {
            ratings
                .iter()
                .filter_map(|rating| rating.credit_for(*axis))
                .map(crate::Weight::get)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let total = sums.iter().sum::<f64>();
    if total == 0.0 {
        return prior_axis_profile(observed_works);
    }
    let weights = axes
        .into_iter()
        .zip(sums)
        .map(|(axis, value)| AxisWeight {
            axis,
            weight: ratio(value / total),
        })
        .collect();
    AxisProfile {
        source: AxisWeightSource::Learned,
        observed_works,
        weights,
    }
}

fn prior_axis_profile(observed_works: usize) -> AxisProfile {
    AxisProfile {
        source: AxisWeightSource::Prior,
        observed_works,
        weights: all_axes()
            .into_iter()
            .map(|axis| AxisWeight {
                axis,
                weight: ratio(AXIS_PRIOR),
            })
            .collect(),
    }
}

const fn all_axes() -> [PersonalAxis; 5] {
    [
        PersonalAxis::Story,
        PersonalAxis::Characters,
        PersonalAxis::WorldBuilding,
        PersonalAxis::VisualDirection,
        PersonalAxis::SoundAndMusic,
    ]
}

fn ratio(value: f64) -> Ratio {
    Ratio::new(value.clamp(0.0, 1.0)).expect("computed profile ratio is finite and bounded")
}
