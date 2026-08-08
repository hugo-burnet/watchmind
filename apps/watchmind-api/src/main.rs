use std::{env, path::PathBuf, time::Duration};
use watchmind_api::{ApiState, router};
use watchmind_infrastructure::{AniListCatalog, CatalogCache, Database};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data =
        env::var_os("WATCHMIND_DATA_DIR").map_or_else(|| PathBuf::from("data"), PathBuf::from);
    let database = Database::open(data.join("watchmind.sqlite")).await?;
    let catalog = AniListCatalog::new(CatalogCache::new(
        data.join("catalog-cache"),
        Duration::from_hours(24),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, router(ApiState::new(database, catalog))).await?;
    Ok(())
}
