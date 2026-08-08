# Scoring explicable V1

Le lot L06 place le calcul derrière `RecommendationEngine::score_candidates`. Le scorer reçoit un profil déjà construit et des candidats déjà récupérés ; il ne décide pas de leur admissibilité.

## Contributions

Le score V1 est uniquement la somme des contributions suivantes :

- pour chaque tag à affinité positive : `poids_catalogue × affinité_apprise × confiance_tag × tag_affinity_weight` ;
- pour chaque tag à affinité négative : la même valeur, multipliée par `negative_tag_penalty_weight` et exposée comme pénalité atomique ;
- proximité avec le meilleur pôle : similarité cosinus avec ses tags dominants, multipliée par `pole_similarity_weight` ;
- prior AniList faible : `((note_globale - 5) / 5) × anilist_prior_weight`.

Le prior est validé dans `[0, 0.25]`. La configuration par défaut se trouve dans `fixtures/config/scoring-v1.json`.

`RecommendationScore` recalcule toujours son total depuis les contributions. Il est donc impossible pour le scorer de produire un total opaque distinct de sa décomposition.

## Explication

`ScoreExplanation` trie les mêmes contributions et conserve au plus les trois signaux positifs les plus forts et les deux contributions négatives les plus importantes. Sa projection texte et sa représentation JSON ne recalculent aucun signal.

Les égalités de score sont départagées par identifiant AniList. À profil, catalogue et configuration identiques, le classement et les explications sont stables.
