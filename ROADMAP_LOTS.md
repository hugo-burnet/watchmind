# WatchMind — découpage en lots d'implémentation

Ce plan transforme les deux documents de conception en lots courts, autonomes et vérifiables. Chaque lot est conçu pour être réalisable dans une tâche Codex focalisée et pour se terminer par un résultat exécutable, des tests verts et un commit propre.

## Règles de découpage

- Un lot poursuit un seul résultat principal.
- Un lot ne mélange pas moteur, persistance, API et interface.
- Chaque lot possède un critère de sortie observable : commande, rapport, endpoint ou écran.
- Les formules sont configurables et testées ; aucun réglage « au feeling ».
- Le score est toujours la somme de contributions explicables.
- Le même jeu de données et la même configuration doivent produire le même classement.
- Anime uniquement jusqu'à validation du moteur V1.
- Aucun embedding, TMDB ou recommandation cross-domain avant le passage du premier verrou.

## Chaîne critique

```text
L00 → L01 → L02 → L03 → L04 → L05 → L06 → L07 → L08 → L09 → L10
                                                              ↓
                                                     VERROU MOTEUR V1
                                                              ↓
L11 → L12 → L13 → L14 → L15 → L16 → L17 → L18 → L19
```

Le chantier **D01 — dataset réel** peut avancer en parallèle de L00 à L05.

---

## Bloc A — Moteur offline

### L00 — Initialiser le workspace Rust

**But :** obtenir un dépôt minimal qui compile sans serveur, base de données ou frontend.

**Livrables :**

- workspace Cargo ;
- crate `recommendation` ;
- binaire `watchmind-cli` ;
- dossiers `fixtures/` et `docs/` ;
- formatage, lint et tests dans une commande locale documentée.

**Terminé quand :** `cargo test --workspace` et `cargo clippy --workspace --all-targets -- -D warnings` passent.

### L01 — Définir les contrats du domaine

**But :** figer le vocabulaire partagé avant d'écrire l'algorithme.

**Livrables :**

- `NormalizedWork`, `TagWeight`, `RatingRecord`, `WatchEvent`, `AspectCredit` ;
- les cinq axes personnels sous forme d'enum ;
- `Contribution`, `ContributionSource` et `RecommendationScore` ;
- types dédiés pour notes, ratios et poids afin de limiter les valeurs invalides ;
- objets métier riches : champs encapsulés, invariants détenus par le type et comportements placés au plus près des données ;
- interface publique minimale, sans trait ou couche de passage hypothétique ;
- sérialisation JSON documentée par des fixtures minimales.

**Terminé quand :** les valeurs invalides sont refusées et les allers-retours JSON sont testés.

### L02 — Importer et valider les fixtures

**But :** charger un dataset local reproductible sans dépendance réseau.

**Livrables :**

- import de `ratings.csv` ;
- import d'un snapshot `catalog.json` contenant les tags AniList pondérés ;
- diagnostics précis pour doublons, note hors plage, drop incohérent et identifiant inconnu ;
- petit dataset synthétique couvrant œuvres aimées, neutres, abandonnées et revues.

**Terminé quand :** `watchmind-cli import-csv ...` affiche un résumé stable et retourne une erreur exploitable sur chaque fixture invalide.

### L03 — Poser le harness et les trois baselines

**But :** disposer de la règle de comparaison avant le moteur sophistiqué.

**Livrables :**

- interface commune de ranking ;
- baseline aléatoire avec seed fixe ;
- baseline score global AniList ;
- baseline recouvrement simple de tags ;
- premières métriques : rang médian, Recall@10, Recall@20 et MRR ;
- rapport texte et JSON.

**Terminé quand :** deux exécutions sur les mêmes fixtures produisent exactement le même rapport.

### L04 — Calculer l'affinité personnelle

**But :** convertir les signaux observés en une cible centrée cohérente.

**Livrables :**

- note centrée sur la moyenne personnelle ;
- bonus de rewatch sous-linéaire avec correction de durée ;
- pénalité d'abandon dépendant de `position / total` ;
- traitement distinct de « bon mais pas pour moi » ;
- paramètres regroupés dans une configuration sérialisable.

**Terminé quand :** les tests de propriété prouvent notamment qu'un rewatch ne réduit jamais l'affinité et qu'un drop précoce pénalise plus qu'un drop tardif.

### L05 — Construire le profil de goût

**But :** apprendre les affinités par tags et représenter plusieurs familles de goûts.

**Livrables :**

- affinité par tag avec shrinkage `Σ(w × y) / (Σw + k)` ;
- niveau de confiance fondé sur le volume et la couverture des données ;
- clustering déterministe des favoris en 2 à 4 pôles ;
- œuvres représentatives et tags dominants de chaque pôle ;
- fallback explicite pour moins de 30 œuvres ;
- apprentissage des poids d'axes seulement si les données sont suffisantes, sinon prior documenté.

**Terminé quand :** le profil synthétique retrouve les pôles attendus et reste stable entre deux exécutions.

### L06 — Scorer avec des contributions exactes

**But :** produire un score explicable par construction.

**Livrables :**

- contribution d'affinité tags ;
- proximité cosinus au meilleur pôle ;
- prior AniList faible ;
- pénalités élémentaires ;
- score final calculé exclusivement à partir des contributions ;
- projection texte des trois raisons positives et deux risques principaux.

**Terminé quand :** `abs(total - somme(contributions))` reste sous l'epsilon défini pour tous les cas de test.

### L07 — Générer les candidats

**But :** séparer clairement retrieval bon marché et scoring.

**Livrables :**

- exclusion des œuvres déjà vues ou blacklistées ;
- filtres de format, année, score minimum et disponibilité des prérequis de franchise ;
- limites configurables ;
- rapport expliquant combien d'items chaque filtre élimine.

**Terminé quand :** aucune œuvre vue ou suite inaccessible ne peut atteindre le scorer dans les fixtures de régression.

### L08 — Diversifier et réserver l'exploration

**But :** éviter un top 10 composé de clones tout en créant de nouveaux signaux.

**Livrables :**

- MMR déterministe ;
- plafonds de franchise, studio et tags dominants ;
- liste finale par défaut de 8 recommandations sûres et 2 paris ;
- paris choisis par incertitude ou désaccord entre pôles, jamais au hasard ;
- libellé expliquant pourquoi chaque pari en est un.

**Terminé quand :** les contraintes sont respectées sans perdre la taille demandée lorsque le catalogue le permet.

### L09 — Compléter l'évaluation

**But :** mesurer le moteur sur des scénarios personnels et non seulement sur des tests unitaires.

**Livrables :**

- leave-one-out sur les favoris ;
- paires de régression telles que `Parasyte > Heroic Age` ;
- backtest temporel lorsque les dates sont disponibles ;
- comparaison automatique avec les trois baselines ;
- rapport Markdown et JSON avec configuration, seed et version du profil.

**Terminé quand :** une régression de classement ou une perte face à la baseline tags fait échouer la commande d'évaluation selon des seuils configurés.

### L10 — Finaliser la CLI V1

**But :** exposer le moteur complet sans infrastructure applicative.

**Commandes :**

```text
import-csv
build-profile
show-poles
recommend
explain
evaluate
leave-one-out
compare-baselines
```

**Terminé quand :** un test end-to-end part des fixtures brutes, génère un profil, classe les candidats, explique le premier résultat et produit le rapport d'évaluation.

---

## D01 — Préparer le dataset réel en parallèle

**Part utilisateur :** fournir ou valider environ 80 anime vus, avec au minimum titre, identifiant AniList et note.

**Part automatisable :** générer le modèle CSV, vérifier les identifiants, enrichir le snapshot catalogue, signaler les champs manquants et produire un rapport de qualité.

Les drops, rewatches, chips et notes libres peuvent être ajoutés progressivement. Le moteur synthétique permet d'avancer avant que D01 soit terminé.

---

## Verrou moteur V1

Le développement applicatif ne commence que si les quatre conditions suivantes sont satisfaites :

1. le moteur dépasse la baseline de recouvrement de tags sur les métriques retenues ;
2. les paires de régression importantes passent ;
3. chaque score est exactement traçable à ses contributions ;
4. le classement est déterministe à dataset et configuration identiques.

Si le verrou échoue, on ouvre un lot expérimental très ciblé sur une seule hypothèse, puis on mesure à nouveau. On ne contourne pas l'échec en construisant l'interface.

---

## Bloc B — Catalogue, stockage et API

### L11 — Intégrer AniList et le cache catalogue

**But :** rechercher et normaliser les anime sans importer tout AniList.

**Livrables :** client GraphQL, mapping vers `NormalizedWork`, cache de snapshots, expiration contrôlée et fixtures de réponses API.

**Terminé quand :** les tests du normaliseur fonctionnent hors ligne et une recherche réelle peut être mise en cache puis relue sans réseau.

### L12 — Ajouter SQLite et SQLx

**But :** persister les données sans coupler la base au moteur.

**Livrables :** migrations, repositories pour œuvres/tags/notes/événements/aspects/préférences, contraintes d'intégrité et métadonnées SQLx offline.

**Terminé quand :** migration depuis une base vide, tests de repository et export/restauration minimal passent.

### L13 — Exposer les flux de bibliothèque

**But :** créer l'API de saisie avant l'API de recommandation.

**Endpoints couverts :** recherche anime, lecture d'une œuvre, ajout/mise à jour de bibliothèque, notes, événements, aspects et commentaire libre.

**Terminé quand :** un test HTTP ajoute une œuvre AniList, la note, enregistre un drop ou rewatch et relit l'état complet.

### L14 — Exposer recommandations et profil

**But :** brancher le moteur validé sur les données persistées.

**Livrables :** endpoints recommandations/profil/évaluation, recalcul atomique, snapshots de profil, historique des scores et contributions.

**Terminé quand :** une modification de note crée une nouvelle version de profil et conserve l'explication historique précédente.

---

## Bloc C — Interface

### L15 — Direction visuelle et socle frontend

**Sujet :** une cartographie personnelle des goûts audiovisuels.

**Audience :** un cinéphile/animephile auto-hébergeur qui veut choisir sa prochaine œuvre en comprenant la recommandation.

**Travail principal de l'interface :** rendre la décision « que regarder ensuite ? » rapide et digne de confiance.

**Direction proposée :** une **carte de goût** où les pôles sont des repères stables et où chaque recommandation montre visuellement son trajet depuis des œuvres déjà aimées. Cette carte est l'unique geste visuel fort ; listes, formulaires et détails restent calmes. Ce choix devra être challengé, décliné en tokens de couleur/type/espace, puis validé sur captures desktop et mobile avant les écrans fonctionnels.

**Livrables :** app React/Vite/TypeScript, tokens, typographies, shell de navigation, primitives accessibles, états vide/chargement/erreur et page laboratoire avec données mockées.

**Terminé quand :** la revue visuelle confirme une identité propre à WatchMind, le focus clavier est visible et `prefers-reduced-motion` est respecté.

### L16 — Recherche, bibliothèque et notation

**But :** couvrir le chemin « trouver → ajouter → regarder → noter ».

**Livrables :** recherche AniList, bibliothèque filtrable, fiche œuvre, statut, note, drop positionnel, rewatch, deux chips maximum et phrase libre optionnelle.

**Terminé quand :** le parcours complet fonctionne au clavier, sur mobile et contre l'API réelle.

### L17 — Accueil et page « Pour toi »

**But :** présenter une décision, pas un score opaque.

**Livrables :** recommandations sûres, deux paris, trois raisons, un risque, pôle source, confiance et feedback léger.

**Terminé quand :** chaque texte affiché est relié à une contribution backend réelle et aucune fausse précision n'est montrée.

### L18 — Profil de goût et historique

**But :** rendre l'apprentissage du système inspectable.

**Livrables :** pôles, œuvres représentatives, affinités positives/négatives, confiance, évolution du profil et historique des recommandations.

**Terminé quand :** la carte de goût possède une alternative textuelle complète et les versions historiques restent consultables.

### L19 — Durcissement et auto-hébergement

**But :** livrer une application personnelle exploitable et récupérable.

**Livrables :** tests end-to-end critiques, budgets de performance, Docker Compose, configuration, authentification si exposition externe, sauvegarde SQLite, restauration, exports CSV/JSON/Markdown et guide d'exploitation.

**Terminé quand :** une installation vierge peut être démarrée, alimentée, sauvegardée, supprimée puis restaurée selon une procédure testée.

---

## Lots futurs, volontairement hors V1

### F01 — Expérience embeddings

Ajouter un signal sémantique local sur commentaires et descriptions, plafonné à 30 % du score et toujours attribué à des voisins connus. Le lot n'est accepté que s'il améliore le rapport L09.

### F02 — TMDB et cross-domain

Ajouter films et séries, définir une taxonomie commune puis mesurer séparément anime→anime, film→film et cross-domain. Aucun mélange silencieux de tags AniList et keywords TMDB.

## Ordre de lancement recommandé

Commencer par **L00**, puis enchaîner strictement jusqu'à **L03**. À ce stade, lancer **D01** si le dataset réel n'est pas déjà prêt. Les lots L04 à L10 peuvent alors être développés avec un filet d'évaluation déjà en place.

Pour les prochaines demandes, il suffit d'indiquer l'identifiant du lot, par exemple :

> Implémente L00 et arrête-toi dès que ses critères de sortie sont satisfaits.
