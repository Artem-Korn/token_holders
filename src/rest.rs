use crate::db::{all_token, token_by_contract_addr};
use crate::validators::{validate_sqlx_response, QueryParamsValidator};
use axum::extract::RawQuery;
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Extension, Json, Router};
use jsonapi::model::*;
use jsonapi::query;
use sqlx::PgPool;

const OFFSET: i64 = 0;
const LIMIT: i64 = 5;

pub fn create_router(connection_pool: PgPool) -> Router {
    Router::new()
        .route("/tokens", get(tokens_handler))
        // .route("/balances", get(balances_handler))
        .layer(Extension(connection_pool))
}

async fn tokens_handler(
    Extension(cp): Extension<PgPool>,
    RawQuery(query): RawQuery,
) -> Result<Response, Response> {
    match query {
        None => get_all_tokens(cp, None).await,
        Some(query) => {
            let query = query::Query::from_params(query.as_str());

            match query.filter {
                Some(_) => get_token_by_contract_addr(cp, query).await,
                None => get_all_tokens(cp, Some(query)).await,
            }
        }
    }
}

async fn get_token_by_contract_addr(cp: PgPool, query: query::Query) -> Result<Response, Response> {
    let mut validator: QueryParamsValidator = QueryParamsValidator::new();

    let contract_addr = validator
        .hashmap_stores_param(&query.filter.unwrap(), "filter[contract_addr]")
        .unwrap_or(Vec::new());

    validator.check_for_errors()?;

    let token =
        validate_sqlx_response(token_by_contract_addr(&cp, contract_addr.first().unwrap()).await)?;
    Ok(Json(token.to_jsonapi_document()).into_response())
}

async fn get_all_tokens(cp: PgPool, query: Option<query::Query>) -> Result<Response, Response> {
    let tokens;

    match query {
        Some(row) => {
            let mut validator = QueryParamsValidator::new();

            let (offset, limit) = validator
                .pagination_parse(row.page)
                .unwrap_or((OFFSET, LIMIT));

            validator.check_for_errors()?;

            tokens = validate_sqlx_response(all_token(&cp, offset, limit).await)?;
        }
        None => {
            tokens = validate_sqlx_response(all_token(&cp, OFFSET, LIMIT).await)?;
        }
    }

    Ok(Json(vec_to_jsonapi_document(tokens)).into_response())
}

// async fn balances_handler(
//     Extension(cp): Extension<PgPool>,
//     RawQuery(query): RawQuery,
// ) -> Result<Response, Response> {
//     let query = query::Query::from_params(query.unwrap_or_default().as_str());
//     let mut query_validator = QueryParamsValidator::new();

//     let filter = query_validator.verify_existence(query.filter, true, "filter");
//     // let include = query_validator.include_parse(query.include);
//     let pagination = query_validator.pagination_parse(query.page);

//     query_validator.check_for_errors()?;

//     // let sort = sort_parse(query.sort);

//     //         if let Ok(holder_id) = holder_id_by_holder_addr(&cp, holder_addr).await {
//     //             if let Ok(balances) = all_balance_by_holder_id(&cp, holder_id, 0, 1, sort).await {
//     //                 Ok(Json(vec_to_jsonapi_document(balances)).into_response())
//     //             } else {
//     //                 Err(Response::builder()
//     //                     .status(StatusCode::NOT_FOUND)
//     //                     .header("X-Custom-Foo", "Bar")
//     //                     .body(Body::from("not found"))
//     //                     .unwrap())
//     //             }

//     return Ok(Response::new("bod2y".into()));
// }
