use std::{env, net::SocketAddr, path::PathBuf, time::Duration};
use watchmind_api::{ApiState, secured_router};
use watchmind_infrastructure::{AniListCatalog, CatalogCache, Database};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data =
        env::var_os("WATCHMIND_DATA_DIR").map_or_else(|| PathBuf::from("data"), PathBuf::from);
    std::fs::create_dir_all(&data)?;
    let database = Database::open(data.join("watchmind.sqlite")).await?;
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if let [command, path] = arguments.as_slice() {
        match command.as_str() {
            "backup" => database.export(path).await?,
            "restore" => database.restore(path).await?,
            _ => return Err(format!("unknown operation: {command}").into()),
        }
        return Ok(());
    }
    if !arguments.is_empty() {
        return Err("usage: watchmind-api [backup|restore <path>]".into());
    }
    let catalog = AniListCatalog::new(CatalogCache::new(
        data.join("catalog-cache"),
        Duration::from_hours(24),
    ));
    let bind = env::var("WATCHMIND_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let bind: SocketAddr = bind.parse()?;
    let token = env::var("WATCHMIND_API_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    let trusted_proxy = env::var("WATCHMIND_TRUST_PROXY").is_ok_and(|value| value == "1");
    if !bind.ip().is_loopback() && token.is_none() && !trusted_proxy {
        return Err("WATCHMIND_API_TOKEN is required outside loopback".into());
    }
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(
        listener,
        secured_router(ApiState::new(database, catalog), token),
    )
    .await?;
    Ok(())
}
