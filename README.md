# WatchMind

WatchMind est un moteur personnel et explicable de recommandation d'anime. Le projet commence volontairement par un moteur Rust utilisable hors ligne avant d'ajouter une base de données, une API ou une interface.

## Prérequis

La toolchain stable et ses composants sont déclarés dans `rust-toolchain.toml`, avec Rust 1.97 comme version minimale du workspace. Après une nouvelle installation de Rust sous Windows, ouvrir un nouveau terminal si `cargo` n'est pas encore reconnu.

### Développement sous Windows

Smart App Control peut bloquer les exécutables non signés que Cargo génère dans
`target/debug/deps`. Pour conserver cette protection tout en exécutant les tests
de manière fiable, utiliser de préférence WSL et placer le dépôt dans son système
de fichiers Linux, par exemple `~/projects/watchmind`, plutôt que sous `/mnt/c`.
La désactivation globale de Smart App Control n'est pas requise pour WatchMind.

## Structure actuelle

```text
apps/watchmind-cli/       binaire CLI
crates/recommendation/    moteur de recommandation isolé
crates/infrastructure/    adaptateurs AniList, cache catalogue et SQLite
docs/                     documentation technique
fixtures/                 datasets et cas de test locaux
scripts/check.ps1         contrôles qualité du workspace
```

## Commandes

```powershell
cargo run -p watchmind-cli
cargo run -p watchmind-cli -- import-csv fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- build-profile fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json --json
cargo run -p watchmind-cli -- show-poles fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- recommend fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- explain 5 fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- evaluate fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json --config fixtures/config/evaluation-v1.json
cargo run -p watchmind-cli -- leave-one-out fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- compare-baselines fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- compare-baselines fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json --json
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Tous les contrôles peuvent être lancés en une fois :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\check.ps1
```

L'application complète peut être lancée avec `docker compose up -d --build` et
sera disponible sur `http://127.0.0.1:8080`. Les sauvegardes, restaurations,
exports et règles d'exposition sont détaillés dans
[`docs/self-hosting.md`](docs/self-hosting.md).

La feuille de route détaillée se trouve dans [`ROADMAP_LOTS.md`](ROADMAP_LOTS.md). Les règles de conception orientée objet et de modules profonds sont définies dans [`docs/architecture-principles.md`](docs/architecture-principles.md).
Le contrat des fixtures et les diagnostics d'import sont décrits dans [`docs/offline-import.md`](docs/offline-import.md).
Le harness, les trois baselines et le contrat des rapports sont décrits dans [`docs/baseline-evaluation.md`](docs/baseline-evaluation.md).
Le calcul d'affinité personnelle et sa configuration V1 sont décrits dans
[`docs/personal-affinity.md`](docs/personal-affinity.md).
La construction déterministe du profil de goût, ses pôles et ses fallbacks sont
décrits dans [`docs/taste-profile.md`](docs/taste-profile.md).
Le score exclusivement dérivé de contributions et la projection des raisons sont
décrits dans [`docs/explainable-scoring.md`](docs/explainable-scoring.md).
Les filtres de retrieval, leur ordre et leur rapport sont décrits dans
[`docs/candidate-generation.md`](docs/candidate-generation.md).
La diversification MMR, les plafonds et les paris d'exploration sont décrits
dans [`docs/diversification.md`](docs/diversification.md).
L'évaluation complète et le verrou configurable du moteur sont décrits dans
[`docs/complete-evaluation.md`](docs/complete-evaluation.md). Les huit commandes
de la CLI V1 et leurs contrats de sortie sont résumés dans
[`docs/cli-v1.md`](docs/cli-v1.md).
Le client AniList, son cache déterministe, les migrations SQLite, repositories
et sauvegardes sont décrits dans
[`docs/catalog-and-storage.md`](docs/catalog-and-storage.md).
L'API de bibliothèque, les recommandations versionnées et leur lancement sont
décrits dans [`docs/http-api.md`](docs/http-api.md).
La direction visuelle, les tokens et le socle React du lot 15 sont décrits dans
[`docs/frontend-foundation.md`](docs/frontend-foundation.md).
