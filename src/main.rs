mod db;
mod evm;
mod rest;
mod utils;
mod validators;

use anyhow::Result;
use std::future::IntoFuture;
use tokio::{join, net::TcpListener};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment vars from .env
    dotenv::dotenv().expect("Error loading .env");

    // Create connection with evm
    let provider = evm::create_provider().await?;

    // Create connection with db
    let connection_pool = db::init_db().await?;

    // Create router and tcp listener
    let app = rest::create_router(connection_pool.clone());
    let listener = TcpListener::bind(format!(
        "{}:{}",
        std::env::var("SERVICE_IP")?,
        std::env::var("SERVICE_PORT")?
    ))
    .await?;

    // Running a service // Update db from evm network
    let (serve, update) = join!(
        axum::serve(listener, app).into_future(),
        evm::update_db(&connection_pool, &provider)
    );

    serve?;
    update?;

    Ok(())
}
