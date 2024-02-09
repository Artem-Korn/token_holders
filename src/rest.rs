use std::{collections::HashSet, vec};

use axum::{
    extract::RawQuery,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use jsonapi::{model::*, query};
use sqlx::PgPool;

use crate::{
    db::{all_balance_by_filter, all_token},
    validators::{validate_sqlx_response, validate_vec_result, QueryParamsValidator},
};

pub fn create_router(connection_pool: PgPool) -> Router {
    Router::new()
        .route("/tokens", get(tokens_handler))
        .route("/balances", get(balances_handler))
        .layer(Extension(connection_pool))
}

pub fn valid_vec_to_jsonapi_document<T: JsonApiModel>(objects: Vec<T>) -> JsonApiDocument {
    let (resources, mut included) = vec_to_jsonapi_resources(objects);

    if let Some(mut vector) = included {
        let mut seen = HashSet::new();

        vector = vector
            .iter()
            .filter(|r| seen.insert((&r.id, &r._type)))
            .cloned()
            .collect();

        included = Some(vector);
    }

    JsonApiDocument::Data(DocumentData {
        data: Some(PrimaryData::Multiple(resources)),
        included,
        ..Default::default()
    })
}

async fn tokens_handler(
    Extension(cp): Extension<PgPool>,
    RawQuery(query_params): RawQuery,
) -> Result<Response, Response> {
    let query_params = query::Query::from_params(query_params.unwrap_or_default().as_str());

    let query_params = QueryParamsValidator::new(query_params)
        .valid_pagination()
        .no_filter()
        .no_include()
        .only_one_required_sort(vec!["symbol", "-symbol"])
        .no_fields()
        .collect_query()?;

    let tokens = validate_sqlx_response(
        all_token(
            &cp,
            query_params.page.unwrap().number,
            query_params.page.unwrap().size,
            query_params.sort.unwrap(),
        )
        .await,
    )?;

    validate_vec_result(&tokens, query_params.page.unwrap().size, "Tokens")?;
    Ok(Json(vec_to_jsonapi_document(tokens)).into_response())
}

async fn balances_handler(
    Extension(cp): Extension<PgPool>,
    RawQuery(query_params): RawQuery,
) -> Result<Response, Response> {
    let query_params = query::Query::from_params(query_params.unwrap_or_default().as_str());

    let query_params = QueryParamsValidator::new(query_params)
        .valid_pagination()
        .only_one_required_filter(vec!["holder.holder_addr", "token.contract_addr"])
        .no_fields()
        .only_one_required_sort(vec!["amount", "-amount"])
        .no_include()
        .collect_query()?;

    let balances = validate_sqlx_response(
        all_balance_by_filter(
            &cp,
            query_params.filter.unwrap(),
            query_params.page.unwrap().number,
            query_params.page.unwrap().size,
            query_params.sort.unwrap(),
        )
        .await,
    )?;

    validate_vec_result(&balances, query_params.page.unwrap().size, "Balances")?;
    Ok(Json(valid_vec_to_jsonapi_document(balances)).into_response())
}
