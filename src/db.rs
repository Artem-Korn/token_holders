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

fn sort_to_sql(sort: Vec<String>) -> String {
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

    // TODO: migrations

    Ok(connection_pool)
}

pub async fn all_token(
    connection_pool: &PgPool,
    number: i64,
    size: i64,
    sort: Vec<String>,
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

pub async fn all_balance_by_filter(
    connection_pool: &PgPool,
    filter: HashMap<String, Vec<String>>,
    number: i64,
    size: i64,
    sort: Vec<String>,
) -> Result<Vec<Balance>, Error> {
    let sql = format!(
        "SELECT 
            CONCAT(balance.holder_id, '_', balance.token_id) AS id, balance.amount::text,
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
