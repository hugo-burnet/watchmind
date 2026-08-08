# Scoring explicable V1

Le lot L06 place le calcul derrière `RecommendationEngine::score_candidates`. Le scorer reçoit un profil déjà construit et des candidats déjà récupérés ; il ne décide pas de leur admissibilité.

## Contributions

Le score V1 est uniquement la somme des contributions suivantes :

- pour chaque tag à affinité positive : `poids_catalogue × affinité_apprise × confiance_tag × tag_affinity_weight` ;
- pour chaque tag à affinité négative : la même valeur, multipliée par `negative_tag_penalty_weight` et exposée comme pénalité atomique ;
- proximité avec le meilleur pôle : similarité cosinus avec ses tags dominants, multipliée par `pole_similarity_weight` ;
- prior AniList : `((note_globale - 5) / 5) × anilist_prior_weight`, libellé « élevé » ou « faible » selon son signe.

Le prior est validé dans `[0, 0.25]`. La configuration par défaut se trouve dans `fixtures/config/scoring-v1.json`.

### Pourquoi une somme et pas une moyenne

Diviser le terme de tags par la masse des tags de l'œuvre semble plus rigoureux : à goût identique, un titre richement tagué ne devrait pas accumuler plus de contributions qu'un titre sobrement tagué. La mesure dit l'inverse. Sur un historique réel de 150 notes et 200 œuvres, la normalisation fait tomber le MRR de `0,282` à `0,145` et le Recall@10 de `0,489` à `0,400`.

Explication retenue : sur AniList, le nombre de tags n'est pas du bruit de catalogage, c'est une conséquence de l'audience. Une œuvre abondamment taguée est une œuvre abondamment vue. La somme captait donc un signal de notoriété, implicitement. Le supprimer sans le remplacer coûte la moitié du MRR ; le remplacer en poussant `anilist_prior_weight` à son maximum revient à faire du moteur un classement de popularité, ce que la V1 refuse.

La décision est donc suspendue, pas tranchée : tant que le harness d'évaluation n'oppose pas au moteur des œuvres que l'utilisateur aurait rejetées, il ne sait pas distinguer « bien recommandé » de « célèbre », et ne peut pas arbitrer.

La similarité œuvre/pôle est calculée par un point d'entrée unique partagé avec le retrieval et la diversification, sur des clés de tags normalisées en minuscules.

`RecommendationScore` recalcule toujours son total depuis les contributions. Il est donc impossible pour le scorer de produire un total opaque distinct de sa décomposition.

## Explication

`ScoreExplanation` trie les mêmes contributions et conserve au plus les trois signaux positifs les plus forts et les deux contributions négatives les plus importantes. Sa projection texte et sa représentation JSON ne recalculent aucun signal.

Les égalités de score sont départagées par identifiant AniList. À profil, catalogue et configuration identiques, le classement et les explications sont stables.
