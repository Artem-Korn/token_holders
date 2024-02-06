use anyhow::Result;
use jsonapi::{api::*, jsonapi_model, model::*};
use serde::{Deserialize, Serialize};
use sqlx::{Decode, Error, FromRow, PgPool};
use std::env;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone, Decode)]
pub struct Token {
    pub id: i32,
    pub contract_addr: String,
    pub last_checked_block: i64,
    pub symbol: String,
    pub decimals: i16,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone, Decode)]
pub struct Holder {
    pub id: i32,
    pub holder_addr: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Balance {
    pub id: String,
    pub amount: String,
    pub token: Token,
    pub holder: Holder,
}

pub async fn init_db() -> Result<PgPool> {
    jsonapi_model!(Token; "token");
    jsonapi_model!(Holder; "holder");
    jsonapi_model!(Balance; "balance"; has one token, holder);

    let database_url = env::var("DATABASE_URL").expect("Error, missing DATABASE_URL in .env");
    let connection_pool = PgPool::connect(&database_url).await?;

    // TODO: migrations

    Ok(connection_pool)
}

pub async fn all_token(
    connection_pool: &PgPool,
    offset: i64,
    limit: i64,
) -> Result<Vec<Token>, Error> {
    let sql = "SELECT token_id AS id,
        encode(contract_addr, 'hex') AS contract_addr, 
        last_checked_block, symbol, decimals FROM token ORDER BY symbol OFFSET $1 LIMIT $2"; // TODO: delete AS

    sqlx::query_as::<_, Token>(sql)
        .bind(offset)
        .bind(limit)
        .fetch_all(connection_pool)
        .await
}

pub async fn token_by_contract_addr(
    connection_pool: &PgPool,
    contract_addr: &String,
) -> Result<Token, Error> {
    let sql = "SELECT token_id AS id,
        encode(contract_addr, 'hex') AS contract_addr, 
        last_checked_block, symbol, decimals FROM token WHERE contract_addr = decode($1, 'hex')";

    sqlx::query_as::<_, Token>(sql)
        .bind(contract_addr)
        .fetch_one(connection_pool)
        .await
}

// pub async fn holder_by_holder_addr(connection_pool: &PgPool, holder_addr: String) -> Result<i32> {
//     let sql = "SELECT holder_id FROM holder WHERE holder_addr = decode($1, 'hex')";

//     Ok(sqlx::query_scalar::<_, i32>(sql)
//         .bind(holder_addr)
//         .fetch_one(connection_pool)
//         .await?)
// }

// pub async fn all_balance_by_holder_id(
//     connection_pool: &PgPool,
//     holder_id: i32,
//     offset: i64,
//     limit: i64,
//     order: String,
// ) -> Result<Vec<Balance>> {
//     let sql = format!("SELECT CONCAT(holder_id, '_', token_id) AS id,
//         amount::text FROM balance WHERE holder_id = $1 ORDER BY balance.amount {} OFFSET $2 LIMIT $3",
//         order);

//     Ok(sqlx::query_as::<_, Balance>(sql.as_str())
//         .bind(holder_id)
//         .bind(offset)
//         .bind(limit)
//         .fetch_all(connection_pool)
//         .await?)
// }

// pub async fn all_balance_by_token(
//     connection_pool: &PgPool,
//     token_id: i32,
//     offset: i64,
//     limit: i64,
//     order: String,
// ) -> Result<Vec<Balance>> {
//     let sql = format!("SELECT CONCAT(holder_id, '_', token_id) AS id,
//         amount::text FROM balance WHERE token_id = $1 ORDER BY balance.amount {} OFFSET $2 LIMIT $3",
//         order);

//     Ok(sqlx::query_as::<_, Balance>(sql.as_str())
//         .bind(token_id)
//         .bind(offset)
//         .bind(limit)
//         .fetch_all(connection_pool)
//         .await?)
// }
