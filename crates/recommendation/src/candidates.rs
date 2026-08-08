use std::{collections::HashSet, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    NormalizedWork, OfflineDataset, Rating, Ratio, RecommendationEngine, ReleaseYear, TasteProfile,
    WorkFormat, WorkId,
};

/// Filtre responsable de l'exclusion d'une œuvre. Le premier filtre applicable gagne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateFilter {
    Seen,
    Blacklisted,
    Format,
    ReleaseYear,
    GlobalScore,
    Availability,
    Prerequisites,
    Limit,
}

/// Paramètres validés et sérialisables du retrieval bon marché.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CandidateRequestData")]
pub struct CandidateRequest {
    blacklisted: Vec<WorkId>,
    formats: Vec<WorkFormat>,
    minimum_year: Option<ReleaseYear>,
    maximum_year: Option<ReleaseYear>,
    minimum_global_score: Option<Rating>,
    require_available: bool,
    require_prerequisites: bool,
    limit: usize,
    popularity_reserve: Ratio,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct CandidateRequestData {
    blacklisted: Vec<WorkId>,
    formats: Vec<WorkFormat>,
    minimum_year: Option<ReleaseYear>,
    maximum_year: Option<ReleaseYear>,
    minimum_global_score: Option<Rating>,
    require_available: bool,
    require_prerequisites: bool,
    limit: usize,
    popularity_reserve: Ratio,
}

impl Default for CandidateRequestData {
    fn default() -> Self {
        Self {
            blacklisted: Vec::new(),
            formats: Vec::new(),
            minimum_year: None,
            maximum_year: None,
            minimum_global_score: None,
            require_available: true,
            require_prerequisites: true,
            limit: 100,
            popularity_reserve: Ratio::new(0.25).expect("default popularity reserve is valid"),
        }
    }
}

impl Default for CandidateRequest {
    fn default() -> Self {
        Self::try_from(CandidateRequestData::default()).expect("default candidate request is valid")
    }
}

impl TryFrom<CandidateRequestData> for CandidateRequest {
    type Error = CandidateError;

    fn try_from(mut data: CandidateRequestData) -> Result<Self, Self::Error> {
        if data.limit == 0 {
            return Err(CandidateError::InvalidConfiguration {
                field: "limit",
                reason: "must be greater than zero",
            });
        }
        data.blacklisted.sort_unstable();
        reject_duplicates("blacklisted", &data.blacklisted)?;
        data.formats.sort_unstable();
        reject_duplicates("formats", &data.formats)?;
        if matches!(
            (data.minimum_year, data.maximum_year),
            (Some(minimum), Some(maximum)) if minimum > maximum
        ) {
            return Err(CandidateError::InvalidConfiguration {
                field: "release_year",
                reason: "minimum_year must not exceed maximum_year",
            });
        }
        Ok(Self {
            blacklisted: data.blacklisted,
            formats: data.formats,
            minimum_year: data.minimum_year,
            maximum_year: data.maximum_year,
            minimum_global_score: data.minimum_global_score,
            require_available: data.require_available,
            require_prerequisites: data.require_prerequisites,
            limit: data.limit,
            popularity_reserve: data.popularity_reserve,
        })
    }
}

fn reject_duplicates<T: PartialEq>(
    field: &'static str,
    values: &[T],
) -> Result<(), CandidateError> {
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CandidateError::InvalidConfiguration {
            field,
            reason: "must not contain duplicates",
        });
    }
    Ok(())
}

/// Stratégie utilisée pour classer les survivants avant la troncature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    /// Aucun profil fourni : classement par score global uniquement.
    Popularity,
    /// Classement par correspondance au goût, avec une réserve de popularité.
    TasteAware,
}

/// Compteurs stables du pipeline de filtrage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandidateReport {
    catalog_count: usize,
    accepted_count: usize,
    retrieval: RetrievalMode,
    taste_selected: usize,
    popularity_selected: usize,
    seen: usize,
    blacklisted: usize,
    format: usize,
    release_year: usize,
    global_score: usize,
    availability: usize,
    prerequisites: usize,
    limit: usize,
}

impl CandidateReport {
    #[must_use]
    pub const fn catalog_count(&self) -> usize {
        self.catalog_count
    }

    #[must_use]
    pub const fn accepted_count(&self) -> usize {
        self.accepted_count
    }

    #[must_use]
    pub const fn retrieval(&self) -> RetrievalMode {
        self.retrieval
    }

    /// Nombre de candidats retenus pour leur correspondance au goût.
    #[must_use]
    pub const fn taste_selected(&self) -> usize {
        self.taste_selected
    }

    /// Nombre de candidats retenus au titre de la réserve de popularité.
    #[must_use]
    pub const fn popularity_selected(&self) -> usize {
        self.popularity_selected
    }

    #[must_use]
    pub const fn eliminated_by(&self, filter: CandidateFilter) -> usize {
        match filter {
            CandidateFilter::Seen => self.seen,
            CandidateFilter::Blacklisted => self.blacklisted,
            CandidateFilter::Format => self.format,
            CandidateFilter::ReleaseYear => self.release_year,
            CandidateFilter::GlobalScore => self.global_score,
            CandidateFilter::Availability => self.availability,
            CandidateFilter::Prerequisites => self.prerequisites,
            CandidateFilter::Limit => self.limit,
        }
    }

    fn increment(&mut self, filter: CandidateFilter) {
        match filter {
            CandidateFilter::Seen => self.seen += 1,
            CandidateFilter::Blacklisted => self.blacklisted += 1,
            CandidateFilter::Format => self.format += 1,
            CandidateFilter::ReleaseYear => self.release_year += 1,
            CandidateFilter::GlobalScore => self.global_score += 1,
            CandidateFilter::Availability => self.availability += 1,
            CandidateFilter::Prerequisites => self.prerequisites += 1,
            CandidateFilter::Limit => self.limit += 1,
        }
    }
}

impl fmt::Display for CandidateReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let retrieval = match self.retrieval {
            RetrievalMode::Popularity => "popularity",
            RetrievalMode::TasteAware => "taste-aware",
        };
        write!(
            formatter,
            "candidates: catalog={} accepted={} retrieval={retrieval} taste={} popularity={} seen={} blacklisted={} format={} year={} score={} unavailable={} prerequisites={} limit={}",
            self.catalog_count,
            self.accepted_count,
            self.taste_selected,
            self.popularity_selected,
            self.seen,
            self.blacklisted,
            self.format,
            self.release_year,
            self.global_score,
            self.availability,
            self.prerequisites,
            self.limit,
        )
    }
}

/// Ensemble admissible transmis au scorer avec la preuve de son filtrage.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CandidateSet {
    works: Vec<NormalizedWork>,
    report: CandidateReport,
}

impl CandidateSet {
    #[must_use]
    pub fn works(&self) -> &[NormalizedWork] {
        &self.works
    }

    #[must_use]
    pub const fn report(&self) -> &CandidateReport {
        &self.report
    }
}

impl RecommendationEngine {
    /// Génère les candidats admissibles sans profil, donc sans personnalisation.
    ///
    /// Les survivantes sont pré-classées par score `AniList` puis identifiant.
    /// Préférer [`RecommendationEngine::generate_candidates_for`] dès qu'un
    /// profil est disponible : ce classement seul plafonne le moteur au haut du
    /// palmarès mondial et rend les pépites inatteignables.
    #[must_use]
    pub fn generate_candidates(
        &self,
        dataset: &OfflineDataset,
        request: &CandidateRequest,
    ) -> CandidateSet {
        self.generate_candidates_for(dataset, request, None)
    }

    /// Génère les candidats admissibles en tenant compte du goût appris.
    ///
    /// Chaque œuvre rejetée est comptée par le premier filtre qui l'élimine.
    /// Les survivantes sont ensuite classées par correspondance au profil, et
    /// une fraction `popularity_reserve` de la limite reste réservée au
    /// meilleur score `AniList` pour conserver une voie « valeur sûre ». Sans
    /// cette réserve, un profil étroit ne verrait plus que ses propres thèmes.
    /// Le résultat est déterministe à dataset, profil et requête identiques.
    #[must_use]
    pub fn generate_candidates_for(
        &self,
        dataset: &OfflineDataset,
        request: &CandidateRequest,
        profile: Option<&TasteProfile>,
    ) -> CandidateSet {
        let seen = dataset
            .ratings()
            .iter()
            .map(crate::RatingRecord::work_id)
            .chain(dataset.events().iter().map(crate::WatchEvent::work_id))
            .collect::<HashSet<_>>();
        let blacklisted = request.blacklisted.iter().copied().collect::<HashSet<_>>();
        let mut report = CandidateReport {
            catalog_count: dataset.catalog().len(),
            accepted_count: 0,
            retrieval: if profile.is_some() {
                RetrievalMode::TasteAware
            } else {
                RetrievalMode::Popularity
            },
            taste_selected: 0,
            popularity_selected: 0,
            seen: 0,
            blacklisted: 0,
            format: 0,
            release_year: 0,
            global_score: 0,
            availability: 0,
            prerequisites: 0,
            limit: 0,
        };

        let mut works = dataset
            .catalog()
            .iter()
            .filter_map(|work| {
                rejected_by(work, request, &seen, &blacklisted).map_or_else(
                    || Some(work.clone()),
                    |filter| {
                        report.increment(filter);
                        None
                    },
                )
            })
            .collect::<Vec<_>>();

        if let Some(profile) = profile {
            let eliminated = rank_by_taste(&mut works, profile, request, &mut report);
            report.limit = eliminated;
            report.accepted_count = works.len();
            return CandidateSet { works, report };
        }

        works.sort_by(by_global_score);
        if works.len() > request.limit {
            report.limit = works.len() - request.limit;
            works.truncate(request.limit);
        }
        report.accepted_count = works.len();
        CandidateSet { works, report }
    }
}

/// Part du score de retrieval attribuée à la proximité au pôle le plus proche.
const RETRIEVAL_POLE_WEIGHT: f64 = 0.5;

fn by_global_score(left: &NormalizedWork, right: &NormalizedWork) -> std::cmp::Ordering {
    match (left.global_score(), right.global_score()) {
        (Some(left), Some(right)) => right.get().total_cmp(&left.get()),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
    .then_with(|| left.id().cmp(&right.id()))
}

/// Classe les survivants par correspondance au goût, puis applique la limite.
///
/// Retourne le nombre d'œuvres écartées par la limite.
fn rank_by_taste(
    works: &mut Vec<NormalizedWork>,
    profile: &TasteProfile,
    request: &CandidateRequest,
    report: &mut CandidateReport,
) -> usize {
    if works.len() <= request.limit {
        works.sort_by(by_global_score);
        report.taste_selected = works.len();
        return 0;
    }

    let eliminated = works.len() - request.limit;
    let reserve = popularity_reserve(request);
    let taste_slots = request.limit - reserve;

    let mut ranked = std::mem::take(works)
        .into_iter()
        .map(|work| {
            let affinity = taste_match(&work, profile);
            (work, affinity)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left, left_match), (right, right_match)| {
        right_match
            .total_cmp(left_match)
            .then_with(|| by_global_score(left, right))
    });

    let mut remaining = ranked.split_off(taste_slots);
    let mut selected = ranked.into_iter().map(|(work, _)| work).collect::<Vec<_>>();
    report.taste_selected = selected.len();

    remaining.sort_by(|(left, _), (right, _)| by_global_score(left, right));
    remaining.truncate(reserve);
    report.popularity_selected = remaining.len();
    selected.extend(remaining.into_iter().map(|(work, _)| work));

    selected.sort_by(by_global_score);
    *works = selected;
    eliminated
}

/// Nombre de places réservées au score global, en gardant au moins une place
/// pour le goût.
fn popularity_reserve(request: &CandidateRequest) -> usize {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let reserve = (request.limit as f64 * request.popularity_reserve.get()).round() as usize;
    reserve.min(request.limit.saturating_sub(1))
}

/// Score de retrieval bon marché d'une œuvre face à un profil.
///
/// Moyenne pondérée des affinités apprises pour ses tags, relevée par sa
/// proximité au pôle le plus proche. Volontairement moins riche que le scoring
/// explicable : il ne sert qu'à décider qui mérite d'être scoré.
fn taste_match(work: &NormalizedWork, profile: &TasteProfile) -> f64 {
    let mass = work
        .tags()
        .iter()
        .map(|tag| tag.weight().get())
        .sum::<f64>();
    let affinity = if mass > 0.0 {
        work.tags()
            .iter()
            .filter_map(|tag| {
                profile.tag_affinity(tag.name()).map(|learned| {
                    tag.weight().get() * learned.value() * learned.confidence().get()
                })
            })
            .sum::<f64>()
            / mass
    } else {
        0.0
    };
    let pole = profile
        .poles()
        .iter()
        .map(|pole| crate::profile::work_pole_similarity(work, pole))
        .fold(0.0_f64, f64::max);
    affinity + RETRIEVAL_POLE_WEIGHT * pole
}

fn rejected_by(
    work: &NormalizedWork,
    request: &CandidateRequest,
    seen: &HashSet<WorkId>,
    blacklisted: &HashSet<WorkId>,
) -> Option<CandidateFilter> {
    if seen.contains(&work.id()) {
        return Some(CandidateFilter::Seen);
    }
    if blacklisted.contains(&work.id()) {
        return Some(CandidateFilter::Blacklisted);
    }
    if !request.formats.is_empty()
        && work
            .format()
            .is_none_or(|format| !request.formats.contains(&format))
    {
        return Some(CandidateFilter::Format);
    }
    if (request.minimum_year.is_some() || request.maximum_year.is_some())
        && work.release_year().is_none_or(|year| {
            request.minimum_year.is_some_and(|minimum| year < minimum)
                || request.maximum_year.is_some_and(|maximum| year > maximum)
        })
    {
        return Some(CandidateFilter::ReleaseYear);
    }
    if request.minimum_global_score.is_some_and(|minimum| {
        work.global_score()
            .is_none_or(|score| score.get() < minimum.get())
    }) {
        return Some(CandidateFilter::GlobalScore);
    }
    if request.require_available && !work.is_available() {
        return Some(CandidateFilter::Availability);
    }
    if request.require_prerequisites
        && work
            .prerequisites()
            .iter()
            .any(|prerequisite| !seen.contains(prerequisite))
    {
        return Some(CandidateFilter::Prerequisites);
    }
    None
}

/// Erreur de validation d'une requête de candidats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateError {
    InvalidConfiguration {
        field: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for CandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { field, reason } => {
                write!(formatter, "invalid candidate request {field}: {reason}")
            }
        }
    }
}

impl Error for CandidateError {}
