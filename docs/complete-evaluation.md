# Évaluation complète du moteur V1

Le rapport L09 rassemble dans une seule exécution :

- le leave-one-out des favoris pour le moteur WatchMind ;
- la comparaison avec les baselines aléatoire, score AniList et recouvrement de tags ;
- les paires de régression personnelles ;
- le backtest temporel lorsque des dates sont fournies ;
- un verdict calculé à partir de seuils configurables.

La configuration de référence est
[`fixtures/config/evaluation-v1.json`](../fixtures/config/evaluation-v1.json).
Elle contient la version du profil, la seed, le seuil qui définit un favori, les
deltas minimaux face à la baseline tags et les observations datées. Les dates
restent propres au harness : le contrat CSV d'import n'est pas modifié.

Pour chaque favori masqué, le profil est reconstruit sans sa note ni ses
événements. Le moteur classe ensuite la cible parmi les œuvres qui ne figurent
pas dans cet historique. Le backtest applique la même procédure en ne gardant
que les notes strictement antérieures à la cible.

Le verrou échoue si Recall@10 ou MRR est inférieur à la baseline tags après
application du delta configuré, ou si une paire de régression est inversée. Le
rapport expose toutes les causes d'échec ; l'adaptateur CLI les traduit en code
de sortie non nul.

Les sorties JSON et Markdown incluent la configuration effective, la seed et la
version du profil. Elles sont stables pour un dataset et une configuration
identiques.
