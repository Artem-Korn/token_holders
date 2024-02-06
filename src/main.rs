mod db;
mod rest;
mod validators;

use crate::{db::init_db, rest::create_router};
use anyhow::Result;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment vars from .env
    dotenv::dotenv().expect("Error loading .env");

    // Create connection with db
    let connection_pool = init_db().await.expect("Error connection to database");

    // Create router and tcp listener
    let app = create_router(connection_pool);
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    // Running a service
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
