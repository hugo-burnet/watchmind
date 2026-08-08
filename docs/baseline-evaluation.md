# Évaluation des baselines

Le module d'évaluation expose un seul point d'entrée :
`evaluate_baselines(&OfflineDataset)`. Il construit les cas, exécute les trois
rankers et calcule toutes les métriques derrière cette interface. La CLI ne
connaît ni les règles de classement ni les formules.

## Construction des cas

Une note personnelle supérieure ou égale à `8.0` définit une œuvre pertinente.
Chaque œuvre pertinente devient successivement la cible d'un cas :

1. la cible est masquée de l'historique ;
2. les autres œuvres pertinentes forment l'historique positif ;
3. les candidats contiennent la cible et les œuvres encore non notées ;
4. les autres œuvres déjà notées sont exclues.

Ce protocole donne au harness une vérité terrain minimale et reproductible. Le
lot L09 l'étendra avec les scénarios personnels, les paires de régression et le
backtest temporel.

## Baselines

- `random` attribue à chaque identifiant une clé pseudo-aléatoire stable avec la
  seed fixe `42`. Il ne dépend ni de l'ordre du catalogue ni de l'horloge.
- `anilist_global_score` trie le score catalogue du plus élevé au plus faible.
  Un score absent passe après tous les scores connus.
- `tag_overlap` calcule une intersection pondérée simple entre les tags du
  candidat et ceux de l'historique positif. Pour un tag partagé, sa contribution
  est le minimum des deux poids ; le meilleur recouvrement de l'historique est
  conservé.

Toutes les égalités sont départagées par identifiant AniList croissant.

## Métriques et rapports

Les rangs commencent à `1`. Le rapport contient le rang médian, `Recall@10`,
`Recall@20` et le rang réciproque moyen (`MRR`). Le JSON conserve aussi le rang
de chaque cible afin de rendre les agrégats vérifiables.

Rapport texte :

```powershell
cargo run -p watchmind-cli -- compare-baselines fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
```

Rapport JSON :

```powershell
cargo run -p watchmind-cli -- compare-baselines fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json --json
```

La configuration, l'ordre des baselines, l'ordre des cibles et le format des
rapports sont stables. Deux exécutions sur les mêmes fichiers produisent donc
exactement les mêmes octets.
