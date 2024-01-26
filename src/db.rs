use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::env;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Token {
    pub token_id: i32,
    pub contract_addr: String,
    pub last_checked_block: i64,
    pub symbol: String,
    pub decimals: i16,
}

// #[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
// pub struct Holder {
//     pub holder_id: i32,
//     pub holder_addr: String,
// }

// #[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
// pub struct Balance {
//     pub holder_id: i32,
//     pub token_id: i32,
//     pub amount: String,
// }

pub async fn init_db() -> Result<PgPool> {
    let database_url = env::var("DATABASE_URL")?;
    let connection_pool = PgPool::connect(&database_url).await?;
    // migrations
    Ok(connection_pool)
}

pub async fn token_by_id(connection_pool: &PgPool, token_id: i32) -> Result<Token> {
    let sql = "SELECT token_id, 
        encode(contract_addr, 'hex') AS contract_addr, 
        last_checked_block, symbol, decimals FROM token WHERE token_id = $1";

    Ok(sqlx::query_as::<_, Token>(sql)
        .bind(token_id)
        .fetch_one(connection_pool)
        .await?)
}
