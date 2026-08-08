# API HTTP locale

Les lots 13 et 14 exposent une API Axum sous `/api`. Elle reste une couche
d'orchestration : les contrats métier vivent dans `watchmind-recommendation` et
SQLite/AniList dans `watchmind-infrastructure`.

## Flux de bibliothèque

- `GET /api/anime/search?q=...` recherche AniList avec le cache local.
- `GET /api/library` liste les œuvres ajoutées avec leur note et leurs événements.
- `GET /api/works/{id}` relit l'œuvre, sa note avec aspects, son commentaire et
  ses événements.
- `PUT /api/library/{id}` ajoute ou met à jour une œuvre et son commentaire.
- `PUT /api/library/{id}/rating` remplace note et aspects, puis recalcule.
- `POST /api/library/{id}/events` ajoute `completed`, `dropped` ou `rewatched`.

## Profil et recommandations

- `GET /api/profile` retourne le dernier snapshot versionné.
- `GET /api/recommendations` retourne les scores et contributions de sa version.
- `GET /api/profile/{version}/recommendations` relit une explication historique.
- `GET /api/evaluation` exécute l'évaluation sur les données locales.

Le profil et tous ses scores sont insérés dans une transaction SQLite unique.
Une erreur annule le recalcul complet et les versions précédentes restent
immuables. Le binaire écoute sur `127.0.0.1:3000`; `WATCHMIND_DATA_DIR` choisit
le répertoire de la base et du cache.
