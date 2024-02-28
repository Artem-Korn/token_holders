mod db;
mod error;
mod evm;
mod rest;
mod utils;
mod validators;

use anyhow::Result;
use axum::Router;
use tokio::{net::TcpListener, sync::mpsc};

/// Wrapper to use try methods
async fn serve_wrapper(listener: TcpListener, app: Router) -> Result<()> {
    Ok(axum::serve(listener, app).await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment vars from .env
    dotenv::dotenv().expect("Error loading variables from .env file");

    // Configuring a fmt subscriber and set it as default
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(tracing::level_filters::LevelFilter::OFF.into())
        .from_env()?
        .add_directive("token_holders=debug".parse()?);

    tracing_subscriber::fmt::fmt()
        .with_env_filter(filter)
        .init();

    // Create connection with evm
    let provider = evm::create_provider()
        .await
        .expect("Error creating a router for evm");

    // Create connection with db
    let connection_pool = db::init_db()
        .await
        .expect("Error connecting to the database");

    // Сreate a channel to add a new token update task
    let (tx, rx) = mpsc::channel(1);

    // Create router and tcp listener
    let app = rest::create_router(connection_pool.clone(), tx);
    let listener = TcpListener::bind(format!(
        "{}:{}",
        std::env::var("SERVICE_IP")?,
        std::env::var("SERVICE_PORT")?
    ))
    .await
    .expect("Error creating TcpListener");

    // Update db from evm network
    // Running a service
    // TODO: Make it as tasks
    tokio::try_join!(
        evm::update_db(connection_pool, provider, rx),
        serve_wrapper(listener, app)
    )?;

    Ok(())
}
