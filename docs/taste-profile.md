# Profil de goût

Le module de profil expose une seule opération :
`build_taste_profile(&OfflineDataset, &TasteProfileConfig)`. Elle calcule les
affinités personnelles de L04, apprend les tags, choisit le mode de profil,
construit les pôles et décide si les axes personnels peuvent être appris.

La configuration V1 sérialisable se trouve dans
[`fixtures/config/taste-profile-v1.json`](../fixtures/config/taste-profile-v1.json).
À dataset et configuration identiques, le résultat et sa sérialisation JSON
sont déterministes.

## Affinité et confiance d'un tag

Pour un tag, son poids catalogue `w`, l'affinité personnelle `y` d'une œuvre et
le shrinkage configurable `k` :

```text
affinité_tag = Σ(w × y) / (Σw + k)
```

Le dénominateur ramène les tags peu observés vers zéro. La confiance combine
deux facteurs qui croissent tous les deux avec les preuves, `n` étant le nombre
d'œuvres portant le tag et `k2` le `tag_confidence_shrinkage` :

```text
confiance_volume = Σw / (Σw + k)
étendue          = n / (n + k2)
confiance_tag    = confiance_volume × étendue
```

L'étendue remplace un facteur `n / nombre_de_notes` qui, lui, était pathologique :
il faisait *baisser* la confiance accordée à un tag chaque fois que l'utilisateur
notait une œuvre ne le portant pas. Les goûts de niche s'éteignaient donc à
mesure que l'historique grandissait. La forme saturante conserve ce que l'ancien
facteur apportait réellement — un tag attesté sur beaucoup d'œuvres mérite plus
de confiance qu'un tag vu deux fois — sans le faire dépendre du volume total.

La valeur par défaut `150` est délibérément grande devant le nombre d'œuvres d'un
historique courant : dans ce régime l'étendue reste quasi linéaire en `n`, ce qui
maximise la discrimination entre tags rares et tags bien attestés. C'est le
réglage qui mesure le mieux sur historique réel (MRR `0,308` contre `0,216` à
`k2 = 0`, sur 150 notes).

Les tags sont indexés sur une clé normalisée en minuscules : deux graphies d'un
même tag partagent leurs preuves au lieu d'apprendre deux affinités séparées.
Les affinités sont triées par cette clé, ce qui permet au scoring de les
retrouver par recherche dichotomique plutôt que par balayage.

La confiance globale du profil combine le volume de l'historique, plafonné au
seuil de clustering, et la part des œuvres observées qui possèdent au moins un
tag.

## Pôles de goût

Avec moins de 30 œuvres par défaut, `ProfileMode::SparseHistory` rend le
fallback observable et produit un seul pôle depuis les favoris disponibles. Si
le volume est suffisant mais qu'il existe moins de deux favoris,
`ProfileMode::SparseFavorites` explicite ce second fallback.

Sinon, les favoris sont regroupés en 2 à 4 pôles. Le nombre croît avec le volume
de l'historique. L'implémentation utilise des vecteurs de tags pondérés, une
initialisation k-means++ gloutonne puis une affectation par similarité cosinus.
Le premier germe est le médoïde et chaque germe suivant minimise le potentiel
résiduel, si bien que les pôles se forment sur les zones denses des favoris.

L'initialisation farthest-first retenait au contraire, par construction, les
favoris les plus atypiques, et chaque germe restait épinglé à son cluster pour
toute la durée de l'algorithme : les pôles s'ancraient durablement sur des
valeurs aberrantes. L'affectation est désormais libre, et un cluster vidé reçoit
le point le plus mal servi par son propre centroïde.

Tous les tie-breaks utilisent l'identifiant AniList ou l'ordre du pôle ; il n'y a
ni aléa ni dépendance à l'ordre d'entrée. Les distances sont calculées une seule
fois par clustering.

Chaque `TastePole` expose son volume, ses tags dominants et jusqu'à trois œuvres
représentatives avec la configuration V1.

## Axes personnels

Les cinq axes utilisent un prior uniforme documenté de `0.2`. Ils ne sont
appris que lorsqu'au moins 10 œuvres possèdent des `AspectCredit` avec la
configuration V1. Au-delà du seuil, chaque poids est la somme des crédits de
l'axe normalisée par la somme de tous les crédits. `AxisWeightSource` indique
toujours `Prior` ou `Learned`, et `observed_works` expose le volume réellement
disponible.

Le constructeur `OfflineDataset::from_parts` permet aux futurs adapters de
stockage de fournir ces crédits sans modifier le format CSV minimal de L02 ; il
valide les références et ordonne les données comme l'import offline.
