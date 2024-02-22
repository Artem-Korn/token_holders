mod db;
mod evm;
mod rest;
mod utils;
mod validators;

use anyhow::Result;
use axum::Router;
use tokio::{net::TcpListener, try_join};

async fn serve_wrapper(listener: TcpListener, app: Router) -> Result<()> {
    Ok(axum::serve(listener, app).await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configuring a fmt subscriber and set it as default
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(tracing::level_filters::LevelFilter::OFF.into())
        .from_env()?
        .add_directive("token_holders=debug".parse()?);

    tracing_subscriber::fmt::fmt()
        .with_env_filter(filter)
        .init();

    // Load environment vars from .env
    dotenv::dotenv().expect("Error loading variables from .env file");

    // Create connection with evm
    let provider = evm::create_provider()
        .await
        .expect("Error creating a router for evm");

    // Create connection with db
    let connection_pool = db::init_db()
        .await
        .expect("Error connecting to the database");

    // Create router and tcp listener
    let app = rest::create_router(connection_pool.clone(), provider.clone());
    let listener = TcpListener::bind(format!(
        "{}:{}",
        std::env::var("SERVICE_IP")?,
        std::env::var("SERVICE_PORT")?
    ))
    .await
    .expect("Error creating TcpListener");

    // Update db from evm network
    // Running a service
    try_join!(
        evm::update_db(connection_pool, provider),
        serve_wrapper(listener, app)
    )?;

    Ok(())
}
