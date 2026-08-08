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

Les survivantes sont préclassées par score AniList décroissant, puis par identifiant, avant l'application de la limite. `CandidateReport` expose le volume catalogue, le nombre accepté et le nombre retiré par chaque filtre, limite comprise.

La configuration par défaut est sérialisée dans `fixtures/config/candidates-v1.json`. Une liste vide de formats et l'absence de bornes d'année ou de score désactivent ces filtres. La disponibilité et les prérequis sont exigés par défaut.

Les anciens snapshots catalogue restent valides : format et année sont optionnels, une œuvre est considérée disponible par défaut et la liste de prérequis est vide par défaut.
