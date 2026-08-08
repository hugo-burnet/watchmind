# Catalogue AniList et stockage SQLite

La crate `watchmind-infrastructure` contient les adaptateurs des lots L11 et
L12. Le moteur `watchmind-recommendation` reste indépendant du réseau, du
cache et de SQLx.

## AniList

`AniListCatalog` exécute une recherche GraphQL paginée (50 résultats maximum),
normalise les scores AniList sur 10, convertit les formats, calcule la durée
totale connue et exclut les tags marqués comme spoilers. L'endpoint est
injectable pour les tests d'intégration.

Chaque réponse brute valide est écrite dans un `CatalogCache`. La clé dépend de
la recherche et de la pagination. L'appelant fournit l'instant Unix courant à
`search`, et `CatalogCache::new` reçoit la durée de validité : l'expiration est
donc déterministe et testable. Une entrée non expirée est relue sans réseau,
puis repasse dans le même normaliseur que la réponse distante.

La fixture `fixtures/anilist/search-anime.json` couvre le mapping hors ligne.

## SQLite

`Database::open` crée la base puis applique les migrations embarquées. La
migration initiale active les clés étrangères et pose des contraintes sur les
identifiants, notes, poids, types d'événements et progressions d'abandon.

La façade fournit six repositories : œuvres, tags, notes, événements, aspects
et préférences. Les œuvres sont stockées à la fois comme contrat JSON complet
et sous forme de tags relationnels ; les autres signaux restent relationnels.
Les écritures composites (œuvre + tags, note + aspects) sont transactionnelles.

`Database::export` produit une sauvegarde JSON versionnée. `restore` valide le
contrat métier et remplace les données dans une transaction unique ; un échec
annule toute la restauration.

Les requêtes utilisent l'API dynamique de SQLx. La compilation est donc
offline sans `DATABASE_URL`; le détail est consigné dans `.sqlx/README.md`.
