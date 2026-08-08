# Affinité personnelle

Le module d'affinité expose une interface unique :
`calculate_affinities(&OfflineDataset, &AffinityConfig)`. Il agrège les notes,
les événements et la durée du catalogue derrière cette interface. Le résultat
reste ordonné par identifiant comme les notes validées du dataset.

La configuration V1 complète et sérialisable se trouve dans
[`fixtures/config/affinity-v1.json`](../fixtures/config/affinity-v1.json). Sa
désérialisation refuse les nombres non finis, les poids négatifs, une référence
de durée nulle, un exposant nul et des bornes de durée inversées.

## Note centrée

Pour une note personnelle `r`, la moyenne personnelle `μ` et l'échelle `s` :

```text
signal_note = (r - μ) / s
```

Une note exactement moyenne est donc neutre, indépendamment de l'échelle de
notation de la personne. La bande neutre évite de donner un sens excessif aux
petits écarts.

L'échelle `s` est apprise sur la dispersion réelle de l'utilisateur, avec un
shrinkage vers la valeur de configuration et un plancher :

```text
s = max(plancher, (n × écart_type + shrinkage × rating_scale) / (n + shrinkage))
```

Une échelle fixe traiterait de la même façon quelqu'un qui note entre 1 et 10 et
quelqu'un qui plafonne entre 7 et 8 : le second produirait des signaux plusieurs
fois plus faibles à goût équivalent, et ses affinités de tags s'en trouveraient
écrasées. Le shrinkage empêche un historique court d'imposer une échelle
aberrante. `AffinityReport::rating_scale` expose l'échelle effectivement
appliquée.

Le cas « bon mais pas pour moi » est explicite lorsque la note atteint
`good_rating_threshold` tout en restant sous la moyenne au-delà de la bande
neutre. Il demeure un signal négatif de goût, mais son amplitude est multipliée
par `good_but_not_for_me_multiplier` pour ne pas confondre qualité reconnue et
rejet franc.

## Rewatch

Pour `n` rewatches, le bonus est sous-linéaire :

```text
bonus_rewatch = rewatch_weight × ln(1 + n) × facteur_durée
facteur_durée = clamp(sqrt(durée / durée_référence), facteur_min, facteur_max)
```

Une durée inconnue donne le facteur neutre `1`. La durée totale optionnelle
`runtime_minutes` appartient à `NormalizedWork` et doit être strictement
positive lorsqu'elle est fournie. Les bornes empêchent un court métrage ou une
très longue série de dominer le signal.

## Abandon

Pour une progression `p = position / total` :

```text
pénalité_drop = -drop_penalty_weight × (1 - p)^drop_curve_exponent
```

La pénalité est ainsi maximale au début et tend vers zéro en fin d'œuvre. Un
rewatch ne peut jamais réduire l'affinité et, à note égale, repousser un drop
vers la fin ne peut jamais la réduire.

Une œuvre abandonnée **sans note** produit également une affinité, portée par la
seule pénalité d'abandon et marquée `RatingSignalKind::Unrated`. C'est souvent le
signal négatif le plus franc d'un historique ; l'ignorer revenait à priver le
profil de ses rejets les plus nets. Ces entrées suivent les notes, triées par
identifiant, et ne sont produites que si l'œuvre existe au catalogue. L'import
CSV exige une note sur chaque ligne : ce cas provient des chemins SQLite et API.

## Résultat explicable

Chaque `PersonalAffinity` expose séparément `rating_signal`, `rewatch_bonus` et
`drop_penalty`. Sa valeur est exactement leur somme. `RatingSignalKind` indique
si le traitement de la note est positif, neutre, négatif, `good_but_not_for_me`
ou `unrated`. Cette décomposition sert directement à l'apprentissage des tags du
lot L05.
