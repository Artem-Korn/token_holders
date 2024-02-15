use anyhow::Result;
use jsonapi::{api::*, jsonapi_model, model::*};
use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, PgPool};
use std::env;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Token {
    #[sqlx(rename = "token_id")]
    pub id: i32,
    pub contract_addr: String,
    pub last_checked_block: i64,
    pub symbol: String,
    pub decimals: i16,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Holder {
    #[sqlx(rename = "holder_id")]
    pub id: i32,
    pub holder_addr: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Balance {
    pub id: String,
    pub amount: String,
    #[sqlx(flatten)]
    pub token: Token,
    #[sqlx(flatten)]
    pub holder: Holder,
}

fn sort_to_sql(sort: Option<Vec<String>>) -> String {
    if let Some(sort) = sort {
        let sort: Vec<_> = sort
            .iter()
            .map(|col_name| {
                if col_name.starts_with("-") {
                    let col_name = &col_name[1..];
                    format!("{} DESC", col_name)
                } else {
                    format!("{} ASC", col_name)
                }
            })
            .collect();

        format!("ORDER BY {}", sort.join(", "))
    } else {
        String::new()
    }
}

fn filter_to_sql(filter: HashMap<String, Vec<String>>) -> String {
    let mut sql_conditions = Vec::new();

    for (key, values) in filter.iter() {
        let condition: String;

        if key.contains("holder_addr") || key.contains("contract_addr") {
            condition = if values.len() == 1 {
                format!("{} = decode('{}', 'hex')", key, values[0])
            } else {
                format!(
                    "{} IN (decode('{}', 'hex'))",
                    key,
                    values.join("', 'hex'), decode('")
                )
            };
        } else {
            condition = if values.len() == 1 {
                format!("{} = '{}'", key, values[0])
            } else {
                format!("{} IN ('{}')", key, values.join("', '"))
            };
        }

        sql_conditions.push(condition);
    }

    format!("WHERE {}", sql_conditions.join(" AND "))
}

pub async fn init_db() -> Result<PgPool> {
    jsonapi_model!(Token; "token");
    jsonapi_model!(Holder; "holder");
    jsonapi_model!(Balance; "balance"; has one token, holder);

    let database_url = env::var("DATABASE_URL").expect("Error, missing DATABASE_URL in .env");
    let connection_pool = PgPool::connect(&database_url).await?;

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Unable to migrate database");

    Ok(connection_pool)
}

pub async fn all_token(
    connection_pool: &PgPool,
    number: i64,
    size: i64,
    sort: Option<Vec<String>>,
) -> Result<Vec<Token>, Error> {
    let sql = format!(
        "SELECT token_id,
        encode(contract_addr, 'hex') AS contract_addr, 
        last_checked_block, symbol, decimals FROM token {} OFFSET $1 LIMIT $2",
        sort_to_sql(sort)
    );

    sqlx::query_as::<_, Token>(&sql)
        .bind(number * size)
        .bind(size)
        .fetch_all(connection_pool)
        .await
}

pub async fn all_token_count(connection_pool: &PgPool) -> Result<i64, Error> {
    let count_sql = "SELECT COUNT(*) FROM token";

    sqlx::query_scalar(count_sql)
        .fetch_one(connection_pool)
        .await
}

pub async fn add_token(
    connection_pool: &PgPool,
    contract_addr: &str,
    last_checked_block: &i64,
    symbol: &str,
    decimals: &i16,
) -> Result<i32, Error> {
    let token_id = sqlx::query_scalar::<_, i32>(
        "INSERT INTO token (contract_addr, last_checked_block, symbol, decimals) 
            VALUES (
                decode($1, 'hex'),
                $2,
                $3,
                $4
            )
            RETURNING token_id",
    )
    .bind(contract_addr)
    .bind(last_checked_block)
    .bind(symbol)
    .bind(decimals)
    .fetch_one(connection_pool)
    .await?;

    Ok(token_id)
}

pub async fn update_token_last_checked_block(
    connection_pool: &PgPool,
    last_checked_block: &i64,
    token_id: &i32,
) -> Result<()> {
    sqlx::query(
        "UPDATE token
                SET last_checked_block = $1
                WHERE token_id = $2",
    )
    .bind(last_checked_block)
    .bind(token_id)
    .execute(connection_pool)
    .await?;

    Ok(())
}

pub async fn add_or_get_holder(connection_pool: &PgPool, holder_addr: &str) -> Result<i32, Error> {
    let holder_id = sqlx::query_scalar::<_, i32>(
        "SELECT holder_id FROM holder WHERE holder_addr = decode($1, 'hex')",
    )
    .bind(holder_addr)
    .fetch_optional(connection_pool)
    .await?;

    match holder_id {
        Some(holder_id) => Ok(holder_id),
        None => {
            sqlx::query_scalar::<_, i32>(
                "INSERT INTO holder (holder_addr) 
                    VALUES (decode($1, 'hex'))
                    RETURNING holder_id",
            )
            .bind(holder_addr)
            .fetch_one(connection_pool)
            .await
        }
    }
}

pub async fn all_balance_by_filter(
    connection_pool: &PgPool,
    filter: HashMap<String, Vec<String>>,
    number: i64,
    size: i64,
    sort: Option<Vec<String>>,
) -> Result<Vec<Balance>, Error> {
    let sql = format!(
        "SELECT 
            CONCAT(balance.holder_id, '_', balance.token_id) AS id, balance.amount::TEXT,
            token.token_id, encode(token.contract_addr, 'hex') AS contract_addr, 
            token.last_checked_block, token.symbol, token.decimals,
            holder.holder_id,
            encode(holder.holder_addr, 'hex') AS holder_addr
        FROM balance
        INNER JOIN holder ON balance.holder_id = holder.holder_id
        INNER JOIN token ON balance.token_id = token.token_id
        {}
        {} OFFSET $1 LIMIT $2",
        filter_to_sql(filter),
        sort_to_sql(sort)
    );

    sqlx::query_as::<_, Balance>(&sql)
        .bind(number * size)
        .bind(size)
        .fetch_all(connection_pool)
        .await
}

pub async fn upsert_balance(
    connection_pool: &PgPool,
    from_holder_id: &i32,
    to_holder_id: &i32,
    token_id: &i32,
    amount: &str,
) -> Result<()> {
    let sql = String::from(
        "INSERT INTO balance (holder_id, token_id, amount) 
        VALUES
            ($1, $3, $4::NUMERIC)
            ($2, $3, $5::NUMERIC)
        ON CONFLICT (holder_id, token_id) DO UPDATE SET amount = balance.amount + EXCLUDED.amount",
    );

    sqlx::query(&sql)
        .bind(from_holder_id)
        .bind(to_holder_id)
        .bind(token_id)
        .bind(amount)
        .bind(format!("-{amount}"))
        .execute(connection_pool)
        .await?;

    Ok(())
}
