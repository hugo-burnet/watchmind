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

Le dénominateur ramène les tags peu observés vers zéro. La confiance d'un tag
combine deux facteurs bornés :

```text
confiance_volume = Σw / (Σw + k)
couverture = œuvres observées avec ce tag / œuvres notées
confiance_tag = confiance_volume × couverture
```

La confiance globale du profil combine de la même façon le volume de
l'historique, plafonné au seuil de clustering, et la part des œuvres notées qui
possèdent au moins un tag.

## Pôles de goût

Avec moins de 30 œuvres par défaut, `ProfileMode::SparseHistory` rend le
fallback observable et produit un seul pôle depuis les favoris disponibles. Si
le volume est suffisant mais qu'il existe moins de deux favoris,
`ProfileMode::SparseFavorites` explicite ce second fallback.

Sinon, les favoris sont regroupés en 2 à 4 pôles. Le nombre croît avec le volume
de l'historique. L'implémentation utilise des vecteurs de tags pondérés, une
initialisation farthest-first puis une affectation par similarité cosinus. Tous
les tie-breaks utilisent l'identifiant AniList ou l'ordre du pôle ; il n'y a ni
aléa ni dépendance à l'ordre d'entrée.

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
