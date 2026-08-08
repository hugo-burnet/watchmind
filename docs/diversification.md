# Diversification et exploration V1

Le lot L08 place la sélection finale derrière
`RecommendationEngine::recommend`. La méthode reçoit le profil et le
`CandidateSet`, effectue le scoring, puis retourne les recommandations sûres
avant les paris d'exploration.

## MMR déterministe

La pertinence des recommandations sûres est le score total ramené dans
`[0, 1]`. À chaque choix, la MMR calcule :

```text
lambda × pertinence - (1 - lambda) × similarité_max_avec_la_sélection
```

La similarité est le cosinus des tags pondérés. Les égalités sont départagées
par pertinence, score brut puis identifiant AniList. La configuration V1 fixe
`lambda` à `0.75`.

La mise à l'échelle écrête les 5 % extrêmes de chaque côté avant de normaliser.
Un min-max brut laissait un unique candidat aberrant comprimer tous les autres
dans un mouchoir de poche, ce qui rendait l'arbitrage MMR dépendant de la
composition du lot plutôt que des écarts réels.

Les similarités deux à deux sont calculées une seule fois par appel, et la
similarité maximale de chaque candidat à la sélection est maintenue de façon
incrémentale au lieu d'être recalculée à chaque tour.

## Plafonds et taille

La liste complète respecte simultanément les plafonds par franchise, studio et
tag dominant. Les tags dominants d'une œuvre sont ses deux tags de poids le
plus élevé par défaut. Une franchise absente et une liste de studios vide ne
créent pas de groupe artificiel.

Avant chaque choix MMR, une recherche de faisabilité vérifie que les places
restantes peuvent encore être remplies. Le moteur évite ainsi qu'un premier
choix glouton réduise la liste alors qu'une combinaison valide existe. Si les
plafonds rendent la taille demandée impossible, il retourne la plus grande
liste faisable.

Cette recherche est encadrée par deux bornes, car son coût explose précisément
quand elle doit conclure par la négative. Une borne de capacité, linéaire,
majore le nombre d'œuvres sélectionnables en traitant chaque famille de
plafonds séparément. À l'intérieur de la descente, le nombre de candidats encore
autorisés fournit une seconde borne : ajouter une œuvre ne peut que resserrer les
plafonds, jamais les rouvrir.

Les valeurs par défaut, sérialisées dans
`fixtures/config/diversification-v1.json`, demandent 8 recommandations sûres et
2 paris, avec au plus 1 œuvre par franchise, 2 par studio et 3 par tag dominant.

## Paris d'exploration

Les places d'exploration sont réservées avant la sélection sûre et classées par
un signal mesuré, jamais par tirage aléatoire :

- **incertitude des tags** : moyenne pondérée de `1 - confiance` ; un tag
  inconnu a une confiance nulle ;
- **désaccord entre pôles** : écart entre la plus forte et la plus faible
  similarité aux pôles, lorsqu'au moins deux pôles ont été appris.

Le signal le plus fort devient un `ExplorationLabel` sérialisé avec son type,
son intensité et un texte explicatif. La MMR s'applique aussi aux paris pour
éviter que les deux places d'exploration soient des clones.

La pertinence d'un pari mélange son intensité et sa pertinence sûre, pondérées
par `exploration_strength_weight` (`0.6` par défaut). Classer les paris sur la
seule étrangeté faisait d'une œuvre très incertaine et très mal scorée un
candidat de premier choix : un pari doit rester un pari raisonnable.
