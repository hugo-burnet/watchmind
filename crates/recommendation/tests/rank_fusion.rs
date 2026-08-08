use watchmind_recommendation::{
    NormalizedWork, OfflineDataset, Rating, RatingRecord, TagWeight, TasteProfileConfig,
    WatchEvent, Weight, WorkId, build_taste_profile, rank_candidates_fused,
};

fn work(id: u32, score: f64, tag: &str) -> NormalizedWork {
    NormalizedWork::new(
        WorkId::new(id).unwrap(),
        format!("Work {id}"),
        Some(Rating::new(score).unwrap()),
        vec![TagWeight::new(tag, Weight::new(1.0).unwrap()).unwrap()],
    )
    .unwrap()
}

/// Les plafonds de diversité doivent s'appliquer au classement livré. Sans cela
/// une tête de liste peut aligner plusieurs saisons d'une même franchise, et
/// n'offrir que trois recommandations réellement distinctes sur dix.
#[test]
fn fused_ranking_spreads_franchises_across_the_head() {
    let saga = (10..=20)
        .map(|id| work(id, 9.0, "Drama").with_franchise("Saga").unwrap())
        .collect::<Vec<_>>();
    let mut catalog = vec![work(1, 8.0, "Drama"), work(2, 7.0, "Mystery")];
    catalog.extend(saga);
    catalog.push(work(30, 6.0, "Drama"));

    let ratings = vec![
        RatingRecord::new(WorkId::new(1).unwrap(), Rating::new(10.0).unwrap(), vec![]).unwrap(),
        RatingRecord::new(WorkId::new(2).unwrap(), Rating::new(9.0).unwrap(), vec![]).unwrap(),
    ];
    let events = [1, 2]
        .into_iter()
        .map(|id| WatchEvent::completed(WorkId::new(id).unwrap()))
        .collect();
    let dataset = OfflineDataset::from_parts(catalog, ratings, events).unwrap();
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();
    let candidates = dataset
        .catalog()
        .iter()
        .filter(|work| work.id().get() >= 10)
        .cloned()
        .collect::<Vec<_>>();

    let ranked = rank_candidates_fused(&dataset, &profile, &candidates).unwrap();
    assert_eq!(ranked.len(), candidates.len(), "aucun candidat n'est perdu");

    // Une seule œuvre de la saga peut franchir le plafond de franchise. Les dix
    // autres sont reportées derrière, donc l'unique œuvre hors franchise remonte
    // immédiatement après elle — alors que sa note mondiale la reléguait loin.
    let head = ranked
        .iter()
        .take(2)
        .map(|recommendation| recommendation.work_id().get())
        .collect::<Vec<_>>();
    assert!(
        head.contains(&30),
        "l'oeuvre hors franchise est promue en tete, obtenu {head:?}"
    );
    assert_eq!(
        head.iter().filter(|id| **id != 30).count(),
        1,
        "une seule saison de la saga occupe la tete, obtenu {head:?}"
    );
}

#[test]
fn fused_ranking_is_deterministic_and_keeps_every_candidate() {
    let catalog = vec![
        work(1, 7.0, "Drama"),
        work(2, 8.0, "Mystery"),
        work(3, 9.0, "Comedy"),
        work(10, 9.5, "Drama"),
        work(11, 7.5, "Mystery"),
        work(12, 8.5, "Comedy"),
    ];
    let ratings = [(1, 10.0), (2, 8.0), (3, 3.0)]
        .into_iter()
        .map(|(id, rating)| {
            RatingRecord::new(
                WorkId::new(id).unwrap(),
                Rating::new(rating).unwrap(),
                vec![],
            )
            .unwrap()
        })
        .collect();
    let events = (1..=3)
        .map(|id| WatchEvent::completed(WorkId::new(id).unwrap()))
        .collect();
    let dataset = OfflineDataset::from_parts(catalog, ratings, events).unwrap();
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default()).unwrap();
    let candidates = dataset.catalog()[3..].to_vec();

    let rank = || {
        rank_candidates_fused(&dataset, &profile, &candidates)
            .unwrap()
            .into_iter()
            .map(|recommendation| recommendation.work_id())
            .collect::<Vec<_>>()
    };
    let first = rank();
    assert_eq!(first, rank());
    assert_eq!(first.len(), candidates.len());
    assert!(candidates.iter().all(|work| first.contains(&work.id())));
}
