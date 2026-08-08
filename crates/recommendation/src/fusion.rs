use std::collections::{HashMap, HashSet};

use crate::{
    DiversificationConfig, NormalizedWork, OfflineDataset, Rating, RatingRecord,
    RecommendationEngine, ScoredRecommendation, ScoringError, TasteProfile, WorkId,
};

/// Poids des quatre experts : moteur explicable, note `AniList`, chevauchement
/// de tags, k-NN résiduel.
///
/// Ces poids ne sont **pas** ceux qui maximisent le leave-one-out. Ce harnais
/// oppose une œuvre que l'utilisateur a choisie et adorée à des œuvres qu'il n'a
/// jamais touchées : tout indicateur de notoriété y gagne d'avance, et son
/// optimum consiste à ne garder que la note `AniList`. Mesuré sur un historique
/// réel, ce classement atteint `MRR 0,848` tout en étant sans usage — la
/// bibliothèque contient déjà l'intégralité des œuvres notées au-dessus de 8,4
/// par `AniList`, donc cet expert n'a plus rien à proposer.
///
/// La pondération privilégie donc le k-NN résiduel, seul expert validé contre
/// la cible qu'il modélise réellement : il prédit l'écart entre note personnelle
/// et note mondiale avec `Pearson 0,62` et `R² 0,38` en leave-one-out, là où un
/// moyennage à plat tombe à `R² 0,034`.
const FUSION_WEIGHTS: [f64; 4] = [1.0, 0.25, 0.25, 2.0];

const RRF_OFFSET: f64 = 60.0;
const RELEVANT_RATING: f64 = 8.0;
const NEIGHBOR_LIMIT: usize = 12;
const SOFTMAX_TEMPERATURE: f64 = 0.12;

/// Classement livré : fusion robuste des rangs du moteur explicable, de la
/// qualité `AniList`, du chevauchement de tags et d'un k-NN sur les résidus de
/// notes. Les scores explicables restent ceux du moteur ; seule leur sélection
/// ordonnée est fusionnée.
///
/// # Errors
/// Propage une erreur si le moteur explicable ne peut pas scorer un candidat.
pub fn rank_candidates_fused(
    dataset: &OfflineDataset,
    profile: &TasteProfile,
    candidates: &[NormalizedWork],
) -> Result<Vec<ScoredRecommendation>, ScoringError> {
    let scored = RecommendationEngine::default().score_candidates(profile, candidates)?;
    let engine = scored
        .iter()
        .map(ScoredRecommendation::work_id)
        .collect::<Vec<_>>();

    let mut global = candidates.iter().collect::<Vec<_>>();
    global.sort_by(|left, right| {
        right
            .global_score()
            .map_or(f64::NEG_INFINITY, Rating::get)
            .total_cmp(&left.global_score().map_or(f64::NEG_INFINITY, Rating::get))
            .then_with(|| left.id().cmp(&right.id()))
    });
    let global = global
        .into_iter()
        .map(NormalizedWork::id)
        .collect::<Vec<_>>();

    let relevant = dataset
        .ratings()
        .iter()
        .filter(|rating| rating.rating().get() >= RELEVANT_RATING)
        .map(RatingRecord::work_id)
        .collect::<HashSet<_>>();
    let relevant_tags = maximum_tag_weights(dataset.catalog(), &relevant);
    let mut tags = candidates.iter().collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        tag_overlap(right, &relevant_tags)
            .total_cmp(&tag_overlap(left, &relevant_tags))
            .then_with(|| left.id().cmp(&right.id()))
    });
    let tags = tags.into_iter().map(NormalizedWork::id).collect::<Vec<_>>();
    let knn = residual_knn(dataset, candidates);

    let rankings = [&engine, &global, &tags, &knn].map(|ranking| rank_map(ranking));
    let weights = FUSION_WEIGHTS;
    let mut fusion = candidates
        .iter()
        .map(|candidate| {
            let score = rankings
                .iter()
                .zip(weights)
                .map(|(ranks, weight)| {
                    let rank = ranks
                        .get(&candidate.id())
                        .copied()
                        .unwrap_or(candidates.len());
                    let rank = u32::try_from(rank).unwrap_or(u32::MAX);
                    weight / (RRF_OFFSET + f64::from(rank))
                })
                .sum::<f64>();
            (candidate.id(), score)
        })
        .collect::<Vec<_>>();
    fusion.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let order = fusion
        .into_iter()
        .enumerate()
        .map(|(index, (id, _))| (id, index))
        .collect::<HashMap<_, _>>();
    let mut scored = scored;
    scored.sort_by_key(|recommendation| order[&recommendation.work_id()]);
    // Les plafonds de diversité ne s'appliquaient sur aucun chemin servi : la
    // tête de liste pouvait aligner plusieurs saisons d'une même franchise.
    Ok(crate::diversification::diversify_head(
        scored,
        candidates,
        &DiversificationConfig::default(),
    ))
}

fn rank_map(ranking: &[WorkId]) -> HashMap<WorkId, usize> {
    ranking
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index + 1))
        .collect()
}

fn maximum_tag_weights(
    catalog: &[NormalizedWork],
    relevant: &HashSet<WorkId>,
) -> HashMap<String, f64> {
    let mut weights = HashMap::<String, f64>::new();
    for tag in catalog
        .iter()
        .filter(|work| relevant.contains(&work.id()))
        .flat_map(NormalizedWork::tags)
    {
        weights
            .entry(tag.name().to_ascii_lowercase())
            .and_modify(|weight| *weight = weight.max(tag.weight().get()))
            .or_insert(tag.weight().get());
    }
    weights
}

fn tag_overlap(candidate: &NormalizedWork, relevant_tags: &HashMap<String, f64>) -> f64 {
    candidate
        .tags()
        .iter()
        .filter_map(|tag| {
            relevant_tags
                .get(&tag.name().to_ascii_lowercase())
                .map(|weight| weight.min(tag.weight().get()))
        })
        .sum()
}

fn residual_knn(dataset: &OfflineDataset, candidates: &[NormalizedWork]) -> Vec<WorkId> {
    let works = dataset
        .catalog()
        .iter()
        .map(|work| (work.id(), work))
        .collect::<HashMap<_, _>>();
    let history = dataset
        .ratings()
        .iter()
        .filter_map(|rating| {
            let work = works.get(&rating.work_id())?;
            let residual = rating.rating().get() - work.global_score().map_or(7.0, Rating::get);
            let vector = tag_vector(work);
            let norm = vector
                .values()
                .map(|weight| weight.powi(2))
                .sum::<f64>()
                .sqrt();
            Some((vector, norm, residual / 5.0))
        })
        .collect::<Vec<_>>();
    let mut scored = candidates
        .iter()
        .map(|candidate| {
            let candidate_vector = tag_vector(candidate);
            let candidate_norm = candidate_vector
                .values()
                .map(|weight| weight.powi(2))
                .sum::<f64>()
                .sqrt();
            let mut nearest = history
                .iter()
                .filter_map(|(vector, norm, residual)| {
                    let similarity = cosine(&candidate_vector, candidate_norm, vector, *norm);
                    (similarity > 0.0).then_some((similarity, *residual))
                })
                .collect::<Vec<_>>();
            nearest.sort_by(|(left, _), (right, _)| right.total_cmp(left));
            nearest.truncate(NEIGHBOR_LIMIT);
            let maximum = nearest.first().map_or(0.0, |(similarity, _)| *similarity);
            let weighted = nearest
                .iter()
                .map(|(similarity, residual)| {
                    let weight = ((similarity - maximum) / SOFTMAX_TEMPERATURE).exp();
                    (weight, weight * residual)
                })
                .collect::<Vec<_>>();
            let mass = weighted
                .iter()
                .map(|(weight, _)| weight)
                .sum::<f64>()
                .max(f64::EPSILON);
            (
                candidate.id(),
                weighted.iter().map(|(_, value)| value).sum::<f64>() / mass,
            )
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    scored.into_iter().map(|(id, _)| id).collect()
}

fn tag_vector(work: &NormalizedWork) -> HashMap<String, f64> {
    work.tags()
        .iter()
        .map(|tag| (tag.name().to_ascii_lowercase(), tag.weight().get()))
        .collect()
}

fn cosine(
    left: &HashMap<String, f64>,
    left_norm: f64,
    right: &HashMap<String, f64>,
    right_norm: f64,
) -> f64 {
    let (shortest, other) = if left.len() <= right.len() {
        (left, right)
    } else {
        (right, left)
    };
    let dot = shortest
        .iter()
        .filter_map(|(tag, weight)| other.get(tag).map(|other_weight| weight * other_weight))
        .sum::<f64>();
    let denominator = left_norm * right_norm;
    if denominator <= f64::EPSILON {
        0.0
    } else {
        dot / denominator
    }
}
