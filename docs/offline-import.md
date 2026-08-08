# Import offline

Le module d'import charge ensemble l'historique personnel et le snapshot du
catalogue afin de vérifier immédiatement leurs références croisées. Son
interface publique est `OfflineDataset::import(ratings, catalog)` ; les chemins
de fichiers restent une responsabilité de la CLI.

## Historique `ratings.csv`

L'en-tête est strict et stable :

```csv
work_id,rating,status,drop_position,total_episodes,rewatches
```

- `work_id` est un identifiant AniList positif présent dans `catalog.json` ;
- `rating` est une note entre 0 et 10 inclus ;
- `status` vaut `completed` ou `dropped` ;
- `drop_position` et `total_episodes` sont vides pour `completed` et tous deux
  renseignés pour `dropped`, avec `drop_position < total_episodes` ;
- `rewatches` est un entier positif ou nul et peut rester vide pour zéro.

Une œuvre ne peut apparaître qu'une fois. Les diagnostics CSV indiquent la
ligne, le champ et la valeur lorsque ces informations sont disponibles.

## Snapshot `catalog.json`

Le fichier est un tableau d'objets `NormalizedWork` au format documenté dans
[`domain-contracts.md`](domain-contracts.md). Les identifiants d'œuvres sont
uniques et les tags AniList utilisent des poids normalisés entre 0 et 1.
`runtime_minutes` est optionnel ; lorsqu'il est connu, il permet de corriger le
signal de rewatch sans modifier le format de l'historique CSV.

## Commande

Depuis la racine du dépôt :

```powershell
cargo run -p watchmind-cli -- import-csv fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
```

La sortie nominale est volontairement compacte et stable :

```text
import ok: catalog=8 ratings=4 completed=3 dropped=1 rewatches=2
```

Les fichiers de `fixtures/invalid/` couvrent les doublons, notes hors plage,
abandons incohérents et identifiants inconnus.
