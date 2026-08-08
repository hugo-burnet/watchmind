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
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Tous les contrôles peuvent être lancés en une fois :

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\check.ps1
```

La feuille de route détaillée se trouve dans [`ROADMAP_LOTS.md`](ROADMAP_LOTS.md). Les règles de conception orientée objet et de modules profonds sont définies dans [`docs/architecture-principles.md`](docs/architecture-principles.md).
