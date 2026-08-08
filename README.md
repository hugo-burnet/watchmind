# WatchMind

WatchMind est un moteur personnel et explicable de recommandation d'anime. Le projet commence volontairement par un moteur Rust utilisable hors ligne avant d'ajouter une base de données, une API ou une interface.

## Prérequis

La toolchain stable et ses composants sont déclarés dans `rust-toolchain.toml`, avec Rust 1.97 comme version minimale du workspace. Après une nouvelle installation de Rust sous Windows, ouvrir un nouveau terminal si `cargo` n'est pas encore reconnu.

## Structure actuelle

```text
apps/watchmind-cli/       binaire CLI
crates/recommendation/    moteur de recommandation isolé
docs/                     documentation technique
fixtures/                 datasets et cas de test locaux
scripts/check.ps1         contrôles qualité du workspace
```

## Commandes

```powershell
cargo run -p watchmind-cli
cargo run -p watchmind-cli -- import-csv fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- compare-baselines fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- compare-baselines fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json --json
cargo run -p watchmind-cli -- recommend fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json
cargo run -p watchmind-cli -- recommend fixtures/synthetic/ratings.csv --catalog fixtures/synthetic/catalog.json --json
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Tous les contrôles peuvent être lancés en une fois :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\check.ps1
```

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
