# Auto-hébergement et récupération

## Démarrage

Docker et Docker Compose suffisent :

```bash
docker compose up -d --build
```

WatchMind écoute uniquement sur `http://127.0.0.1:8080` par défaut. Le port se
change avec `WATCHMIND_PORT`. Les données vivent dans le volume
`watchmind-data`; AniList reste la seule dépendance réseau.

Pour une exposition derrière un reverse proxy, définir un secret long dans
`WATCHMIND_API_TOKEN`. Nginx l'injecte vers l'API. Ne pas publier directement le
port 3000. Hors Docker, l'API refuse une écoute non locale sans ce token.

## Sauvegarde et restauration

Le format JSON versionné contient œuvres, bibliothèque, notes, aspects,
événements, préférences, profils et recommandations historiques.

```bash
docker compose exec api watchmind-api backup /data/backup.json
docker compose exec api watchmind-api restore /data/backup.json
docker compose cp api:/data/backup.json ./watchmind-backup.json
```

Une restauration remplace les données applicatives dans une transaction. Faire
une sauvegarde récente avant l'opération.

## Exports lisibles

```bash
curl -o library.json http://127.0.0.1:8080/api/export?format=json
curl -o library.csv http://127.0.0.1:8080/api/export?format=csv
curl -o library.md http://127.0.0.1:8080/api/export?format=markdown
```

Ces exports sont destinés à la lecture et aux tableurs. Seul le backup JSON
versionné permet une restauration complète.

## Vérification et reprise

`GET /api/health` sert de sonde. Le script `scripts/smoke-self-host.sh` construit
une installation isolée, attend sa santé, sauvegarde, restaure et vérifie un
export, puis détruit son volume de test.

En cas d'incident : arrêter Compose, conserver une copie du volume et du backup,
redémarrer, restaurer le backup, puis vérifier la bibliothèque et les versions
du profil avant toute nouvelle notation.

## Budgets

`npm run build && npm run budget` limite chaque bundle JavaScript à 250 Ko et
chaque feuille CSS à 40 Ko non compressés. Ces seuils sont exécutés par le
script de contrôle du dépôt.
