# Contrats du domaine

Le module `watchmind_recommendation` expose des objets métier immuables. Leurs
constructeurs et leur désérialisation JSON appliquent exactement les mêmes
invariants : un fichier externe ne peut donc pas créer un état qu'un appelant
Rust ne pourrait pas construire.

## Valeurs numériques

| Type | Invariant |
|---|---|
| `WorkId` | entier AniList strictement positif |
| `RuntimeMinutes` | durée totale strictement positive en minutes |
| `Rating` | nombre fini dans `[0, 10]` |
| `Ratio` | nombre fini dans `[0, 1]` |
| `Weight` | nombre fini dans `[0, 1]` |
| `ScoreDelta` | nombre signé fini |
| `DropProgress` | `total > 0` et `position < total` |

Un `TagWeight` et un `AspectCredit` exigent en plus un poids strictement
positif. Les tags d'une `NormalizedWork` sont uniques sans tenir compte de la
casse. Les crédits d'un `RatingRecord` sont uniques par axe.

## Axes personnels

Les cinq valeurs JSON de `PersonalAxis` sont `story`, `characters`,
`world_building`, `visual_direction` et `sound_and_music`.

## Formes JSON stables

Les fichiers de référence se trouvent dans [`fixtures/domain`](../fixtures/domain) :

- `normalized-work.json` décrit le contrat catalogue minimal, dont la durée
  totale optionnelle `runtime_minutes` ;
- `rating-record.json` associe une note et des crédits d'aspects à une œuvre ;
- `watch-events.json` documente les événements `completed`, `dropped` et
  `rewatched` ;
- `recommendation-score.json` documente les sources et contributions.

`RecommendationScore.total` est une projection lisible de la somme de
`contributions[].value`. Le total est recalculé à la construction et une valeur
JSON incohérente est refusée avec une tolérance de `1e-9`.
