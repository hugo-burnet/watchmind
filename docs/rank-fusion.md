# Fusion de rangs multi-experts

Le classement livré utilise une **Reciprocal Rank Fusion** (RRF) déterministe :

1. le score explicable historique de WatchMind ;
2. la note globale AniList ;
3. le chevauchement maximal des tags avec les favoris ;
4. un k-NN résiduel qui propage l'écart entre la note personnelle et la note
   AniList depuis les 12 œuvres les plus proches (softmax, température 0,12).

Les rangs sont fusionnés avec `1 / (60 + rang)`. Les poids livrés sont
respectivement `1 / 0,25 / 0,25 / 2`. La fusion de rangs évite de comparer
directement quatre échelles de scores incompatibles. Le score et les motifs
affichés restent ceux du moteur explicable ; la fusion décide de l'ordre.

## Pourquoi ces poids ne maximisent pas le leave-one-out

L'ablation sur historique réel est sans ambiguïté :

| Pondération | Recall@10 | MRR |
|---|---:|---:|
| Fusion complète `1/1/.25/1` | 84,4 % | 0,431 |
| Fusion **sans** `AniList` | 44,4 % | 0,245 |
| Fusion **sans** k-NN résiduel | 93,3 % | 0,563 |
| `AniList` seul | 93,3 % | 0,848 |

Le harnais réclame donc de supprimer les experts personnels. On ne le suit pas,
parce qu'il mesure la mauvaise chose : il oppose une œuvre que l'utilisateur a
choisie et adorée à des œuvres qu'il n'a jamais touchées, si bien qu'un tri par
notoriété la retrouve d'avance. Élargir le vivier de 200 à 1313 œuvres ne change
rien — la baseline `AniList` reste identique au millième, parce que la
bibliothèque contient **déjà toutes** les œuvres notées au-dessus de `8,4`. Cet
expert n'a plus rien à proposer à cet utilisateur, tout en gagnant la mesure.

À l'inverse, le k-NN résiduel évalué contre la cible qu'il modélise vraiment —
l'écart entre note personnelle et note mondiale — obtient `Pearson 0,62` et
`R² 0,38` en leave-one-out sur 150 notes, contre `R² 0,034` pour un moyennage à
plat. C'est le seul expert dont la valeur soit établie, d'où son poids double.

La vérification se fera en ligne, via `GET /api/recommendations/impact`.

Le graphe de recommandations communautaires AniList a été évalué puis rejeté
comme ranker : Recall@10 `20,0 % -> 22,2 %`, mais MRR `0,154 -> 0,113`. Il
n'est pas nécessaire au moteur livré.

## Mesure personnelle

Manifeste fixe de 257 œuvres, 45 cibles leave-one-out :

| Moteur | Recall@10 | Recall@20 | Rang médian | MRR |
|---|---:|---:|---:|---:|
| WatchMind précédent | 26,7 % | 40,0 % | 30 | 0,183 |
| Fusion livrée | **48,9 %** | **71,1 %** | **12** | **0,323** |

Sur deux plis déterministes, les deux moitiés sélectionnent la même variante.
Sur le pli tenu à l'écart, elle atteint respectivement `50,0 %` contre `23,3 %`
et `46,7 %` contre `33,3 %` en Recall@10. Elle améliore 39 cibles, en dégrade
une et laisse cinq rangs inchangés.

Ces chiffres ne prouvent pas une généralisation à d'autres utilisateurs. Ils
montrent que le gain ne vient pas seulement d'un réglage choisi sur une moitié
de cet historique.

## Pourquoi pas un réseau neuronal local

LightGCN simplifie efficacement la propagation sur un graphe utilisateur-item,
mais nécessite une matrice multi-utilisateurs pour apprendre ses embeddings.
EASE a la même dépendance aux interactions collectives. WatchMind ne possède
qu'un historique personnel : entraîner ces modèles localement donnerait plus de
paramètres, pas plus d'information. La fusion et le k-NN restent CPU-only,
instantanés sur quelques centaines ou milliers de candidats et adaptés à un PC
portable.

Sources : [LightGCN](https://arxiv.org/abs/2002.02126),
[EASE](https://arxiv.org/abs/1905.03375),
[état de l'art sur le biais de popularité](https://arxiv.org/abs/2308.01118),
[documentation des recommandations AniList](https://docs.anilist.co/reference/object/recommendation).
