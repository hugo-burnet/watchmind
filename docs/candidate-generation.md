# Génération des candidats V1

Le lot L07 sépare le retrieval bon marché du scoring. `RecommendationEngine::generate_candidates` retourne un `CandidateSet`, puis son contenu peut être transmis à `score_candidates`.

## Filtres et ordre

Chaque œuvre est attribuée au premier filtre qui l'élimine, dans cet ordre :

1. déjà vue ou notée ;
2. blacklistée ;
3. format non autorisé ou inconnu lorsqu'un format est demandé ;
4. année hors plage ou inconnue lorsqu'une plage est demandée ;
5. score AniList sous le minimum ou inconnu lorsqu'un minimum est demandé ;
6. indisponible ;
7. prérequis de franchise non vu.

## Classement avant troncature

Le classement des survivantes dépend de la présence d'un profil.

`generate_candidates_for(dataset, request, Some(profile))` classe par correspondance au goût : moyenne pondérée des affinités apprises pour les tags de l'œuvre, relevée de moitié par sa similarité au pôle le plus proche. Une fraction `popularity_reserve` de la limite reste réservée aux meilleurs scores AniList, ce qui conserve une voie « valeur sûre » et évite qu'un profil étroit ne se replie sur ses seuls thèmes.

`generate_candidates(dataset, request)` conserve l'ancien comportement — score AniList décroissant puis identifiant — et n'est correct que sans profil disponible. Classer uniquement par score global plafonne le moteur au haut du palmarès mondial : une œuvre parfaitement alignée sur le goût mais mal notée globalement ne franchit jamais la limite, quel que soit le raffinement du scoring en aval.

`CandidateReport` expose le volume catalogue, le nombre accepté, le mode de retrieval retenu, la répartition goût/popularité, et le nombre retiré par chaque filtre, limite comprise.

La configuration par défaut est sérialisée dans `fixtures/config/candidates-v1.json`. Une liste vide de formats et l'absence de bornes d'année ou de score désactivent ces filtres. La disponibilité et les prérequis sont exigés par défaut.

Les anciens snapshots catalogue restent valides : format et année sont optionnels, une œuvre est considérée disponible par défaut et la liste de prérequis est vide par défaut.
