# SQLx offline

Les requêtes de l'adaptateur SQLite utilisent l'API dynamique `sqlx::query` et
les migrations sont embarquées par `sqlx::migrate!`. Elles ne nécessitent donc
pas de connexion à une base lors de la compilation et ne produisent aucun
fichier `.sqlx/query-*.json`. Ce répertoire documente explicitement ce mode
offline ; les migrations sous `crates/infrastructure/migrations` sont la source
de vérité du schéma.
