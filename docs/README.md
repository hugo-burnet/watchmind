# Documentation technique

Ce dossier accueillera les décisions d'architecture, formats de fixtures et rapports d'évaluation du moteur.

Principes à préserver :

- moteur offline avant l'application ;
- score égal à la somme de contributions explicables ;
- comparaison obligatoire avec les baselines ;
- dépendances orientées vers le cœur du domaine ;
- résultats déterministes à configuration et dataset identiques.

Les règles d'organisation du code sont détaillées dans [`architecture-principles.md`](architecture-principles.md).
Les invariants et formats JSON sont documentés dans [`domain-contracts.md`](domain-contracts.md).
Le format du dataset offline est documenté dans [`offline-import.md`](offline-import.md).
