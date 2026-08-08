# Évaluation complète du moteur V1

Le rapport L09 rassemble dans une seule exécution :

- le leave-one-out des favoris pour le moteur WatchMind ;
- la survie des cibles à travers le pipeline réellement livré ;
- la comparaison avec les baselines aléatoire, score AniList et recouvrement de tags ;
- les paires de régression personnelles ;
- le backtest temporel lorsque des dates sont fournies ;
- un verdict calculé à partir de seuils configurables.

La configuration de référence est
[`fixtures/config/evaluation-v1.json`](../fixtures/config/evaluation-v1.json).
Elle contient la version du profil, la seed, le seuil qui définit un favori, les
deltas minimaux face à la baseline tags et les observations datées. Les dates
restent propres au harness : le contrat CSV d'import n'est pas modifié.

Le seuil qui définit un favori est borné par la distribution personnelle :
`min(seuil_configuré, quantile 0,75 des notes)`. Un seuil purement absolu
refusait l'historique d'un noteur sévère qui ne dépasse jamais 7, alors que tout
le reste du moteur raisonne en écart à la moyenne personnelle. Un noteur généreux
conserve le seuil absolu tel quel.

Pour chaque favori masqué, le profil est reconstruit sans sa note ni ses
événements. Le moteur classe ensuite la cible parmi les œuvres qui ne figurent
pas dans cet historique. Le backtest applique la même procédure en ne gardant
que les notes strictement antérieures à la cible.

## Pipeline livré

Les métriques de rang classent tout le catalogue : elles ignorent donc la
troncature du retrieval et la diversification, c'est-à-dire les deux endroits où
le produit perd le plus. Le bloc `pipeline` mesure ce que l'utilisateur reçoit
vraiment, en rejouant `generate_candidates_for` puis `recommend` sur chaque cas :

- `retrieval_recall` : part des cibles qui franchissent le retrieval borné ;
- `list_recall` : part des cibles qui atteignent la liste finale.

L'écart entre les deux chiffres est le coût de la sélection. Ces valeurs sont
rapportées, pas encore transformées en verrou.

Le verrou échoue si Recall@10 ou MRR est inférieur à la baseline tags après
application du delta configuré, ou si une paire de régression est inversée. Le
rapport expose toutes les causes d'échec ; l'adaptateur CLI les traduit en code
de sortie non nul.

Les sorties JSON et Markdown incluent la configuration effective, la seed et la
version du profil. Elles sont stables pour un dataset et une configuration
identiques.
