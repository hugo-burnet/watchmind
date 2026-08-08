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

AniList reste le catalogue canonique : WatchMind ne tente pas de le recopier
dans SQLite. Pour recommander, l'API construit un pool virtuel à la demande à
partir de deux pages généralistes et de deux pages pour chacun des quatre tags
positifs les plus fiables du profil. Les doublons, œuvres déjà vues et œuvres
masquées sont retirés avant le scoring. Ce mélange évite de limiter le moteur
au seul sommet du classement mondial tout en gardant une réserve populaire.

Les réponses GraphQL brutes ne sont qu'un cache expirant de 24 heures. Elles
peuvent être supprimées sans perte de données personnelles et sont renouvelées
automatiquement ; SQLite ne conserve durablement que la bibliothèque, les
signaux personnels et les instantanés explicables.

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

La migration `0003_rating_dates.sql` ajoute une date nullable aux notes. Les
nouvelles notes sont horodatées atomiquement avec leur instantané de profil ;
les anciennes restent sans date. Le format de sauvegarde version 3 conserve
ces dates et la restauration reste compatible avec les sauvegardes 1 et 2.

## Précision des flottants persistés

Le workspace active la feature `float_roundtrip` de `serde_json`. Sans elle, la
lecture d'un flottant dont la représentation courte demande dix-sept chiffres
significatifs se décale d'un ULP. Les conséquences n'étaient pas cosmétiques :
une explication relue cessait d'être bit à bit celle qui avait été calculée, et
l'API créait une nouvelle version de profil à chaque appel alors que rien
n'avait changé. `crates/infrastructure/tests/snapshot_precision.rs` verrouille
l'aller-retour sur le vrai chemin de persistance.

Les requêtes utilisent l'API dynamique de SQLx. La compilation est donc
offline sans `DATABASE_URL`; le détail est consigné dans `.sqlx/README.md`.
