use std::collections::HashMap;

use axum::extract::RawQuery;
use axum::{http::StatusCode, routing::get, Extension, Json, Router};
use jsonapi::model::{vec_to_jsonapi_document, JsonApiDocument};
use jsonapi::query::{self, PageParams};
use sqlx::PgPool;

use crate::db::{
    all_balance_by_holder_id, all_balance_by_token, all_token, holder_id_by_holder_addr,
    token_id_by_contract_addr,
};

#[derive(Debug)]
enum Filter {
    Token(String),
    Holder(String),
}

#[derive(Debug)]
enum Include {
    Token,
    Holder,
}

fn pagination_parse(page_params: Option<PageParams>) -> Result<(i64, i64), StatusCode> {
    match page_params {
        Some(pagination) => {
            if pagination.number >= 0 && pagination.size >= 0 {
                Ok((pagination.number * pagination.size, pagination.size))
            } else {
                Err(StatusCode::BAD_REQUEST)
            }
        }
        None => Ok((0, 0)),
    }
}

fn filter_parse(filter_params: Option<HashMap<String, Vec<String>>>) -> Result<Filter, StatusCode> {
    match filter_params {
        Some(filter) => match (filter.get("token"), filter.get("holder")) {
            (Some(token), None) => Ok(Filter::Token(token.first().unwrap().clone())),
            (None, Some(holder)) => Ok(Filter::Holder(holder.first().unwrap().clone())),
            (_, _) => Err(StatusCode::BAD_REQUEST),
        },
        None => Err(StatusCode::BAD_REQUEST),
    }
}

fn include_parse(include_params: Option<Vec<String>>) -> Result<Include, StatusCode> {
    match include_params {
        Some(include) => match (
            include.contains(&String::from("token")),
            include.contains(&String::from("holder")),
        ) {
            (true, false) => Ok(Include::Token),
            (false, true) => Ok(Include::Holder),
            (_, _) => Err(StatusCode::BAD_REQUEST),
        },
        None => Err(StatusCode::BAD_REQUEST),
    }
}

fn sort_parse(sort_params: Option<Vec<String>>) -> String {
    match sort_params {
        Some(sort) => match (
            sort.contains(&String::from("-amount")),
            sort.contains(&String::from("amount")),
        ) {
            (true, false) => String::from("DESC"),
            (false, true) => String::from("ASC"),
            (_, _) => String::from("ASC"),
        },
        None => String::from("ASC"),
    }
}

pub fn create_router(connection_pool: PgPool) -> Router {
    Router::new()
        .route("/tokens", get(tokens_handler))
        .route("/balances", get(balances_handler))
        .layer(Extension(connection_pool))
}

async fn tokens_handler(
    Extension(cp): Extension<PgPool>,
    RawQuery(query): RawQuery,
) -> Result<Json<JsonApiDocument>, StatusCode> {
    let query = query::Query::from_params(query.unwrap_or_default().as_str());
    let (offset, limit) = pagination_parse(query.page)?;

    if let Ok(tokens) = all_token(&cp, offset, limit).await {
        Ok(Json(vec_to_jsonapi_document(tokens)))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn balances_handler(
    Extension(cp): Extension<PgPool>,
    RawQuery(query): RawQuery,
) -> Result<Json<JsonApiDocument>, StatusCode> {
    let query = query::Query::from_params(query.unwrap_or_default().as_str());
    let filter = filter_parse(query.filter)?;
    let include = include_parse(query.include)?;
    let (offset, limit) = pagination_parse(query.page)?;
    let sort = sort_parse(query.sort);

    match (filter, include) {
        (Filter::Holder(holder_addr), Include::Token) => {
            if let Ok(holder_id) = holder_id_by_holder_addr(&cp, holder_addr).await {
                if let Ok(balances) =
                    all_balance_by_holder_id(&cp, holder_id, offset, limit, sort).await
                {
                    Ok(Json(vec_to_jsonapi_document(balances)))
                } else {
                    Err(StatusCode::NOT_FOUND)
                }
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        (Filter::Token(contract_addr), Include::Holder) => {
            if let Ok(token_id) = token_id_by_contract_addr(&cp, contract_addr).await {
                if let Ok(balances) = all_balance_by_token(&cp, token_id, offset, limit, sort).await
                {
                    Ok(Json(vec_to_jsonapi_document(balances)))
                } else {
                    Err(StatusCode::NOT_FOUND)
                }
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        (_, _) => Err(StatusCode::BAD_REQUEST),
    }
}
