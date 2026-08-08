# Principes d'architecture

## Intention

WatchMind utilise une conception orientée objet poussée pour concentrer la connaissance métier et réduire la quantité de code qu'un humain ou un agent doit lire avant de modifier un comportement.

En Rust, cela signifie **objets métier riches, encapsulation, composition et modules profonds**. Cela ne signifie pas reproduire l'héritage classique avec des traits et des couches sans comportement.

## Règles obligatoires

### 1. Les objets métier protègent leurs invariants

- Pas de `struct` métier publique remplie de champs librement modifiables.
- La construction passe par un constructeur validant ou une conversion fallible.
- Une opération liée à un concept vit sur ce concept.
- Les états impossibles doivent être difficiles ou impossibles à représenter.

Exemples futurs : `PersonalRating`, `DropProgress`, `TagWeight`, `TasteProfile` et `RecommendationScore` possèdent leurs validations et comportements.

### 2. Les modules sont profonds

Un module expose une petite interface et cache une implémentation substantielle. Le lecteur doit pouvoir utiliser le moteur en lisant les exports de `lib.rs` et la documentation de l'interface, sans parcourir l'algorithme interne.

La façade cible du cœur restera volontairement étroite :

```rust,ignore
let profile = engine.build_profile(history)?;
let recommendations = engine.recommend(&profile, candidates, request)?;
let report = engine.evaluate(history, catalog, evaluation)?;
```

Les détails de normalisation, clustering, scoring, contributions et diversification restent derrière cette interface.

### 3. Composition avant héritage simulé

- Utiliser des structs qui collaborent plutôt qu'une hiérarchie de traits.
- Introduire un trait seulement à une vraie seam où plusieurs adapters existent réellement.
- Ne pas créer de trait pour chaque struct « au cas où ».
- Ne pas multiplier les factories, managers, services et wrappers de passage.

### 4. Dépendances injectées, résultats retournés

- Les dépendances variables sont reçues par construction ou par méthode.
- Le domaine ne crée pas lui-même de client AniList, de connexion SQLite ou d'horloge système.
- Les calculs retournent des valeurs ; les adapters prennent en charge les effets externes.
- Les tests utilisent la même interface que les appelants réels.

### 5. Lecture locale optimisée

Chaque capacité doit être compréhensible avec au plus :

1. l'interface publique du module ;
2. le fichier d'implémentation concerné ;
3. ses tests proches.

Les exports publics sont centralisés. Les invariants et erreurs sont documentés sur l'interface. Les types liés restent regroupés dans un même module plutôt que dispersés artificiellement dans un fichier par classe.

### 6. Test de suppression

Avant d'ajouter une couche, imaginer sa suppression :

- si sa complexité disparaît, la couche est probablement un simple passage et doit être supprimée ;
- si sa logique se répandrait dans plusieurs appelants, le module gagne sa place par la localité qu'il apporte.

## Conséquence pour les prochains lots

L01 définira d'abord des objets métier riches et leurs invariants. L06 placera l'ensemble du calcul derrière une façade de moteur compacte. Les traits arriveront avec de vrais adapters, notamment stockage mémoire/SQLite ou catalogue fixture/AniList, pas avant.
