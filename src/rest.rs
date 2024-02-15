use axum::{
    extract::{self, RawQuery},
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use jsonapi::{model::*, query};
use sqlx::PgPool;
use std::vec;

use crate::{
    db, utils,
    validators::{self, QueryParamsValidator as QPV},
};

pub fn create_router(connection_pool: PgPool) -> Router {
    Router::new()
        .route("/tokens", get(get_tokens).post(post_token))
        .route("/balances", get(get_balances))
        .layer(Extension(connection_pool))
}

async fn get_tokens(
    Extension(cp): Extension<PgPool>,
    RawQuery(query_params): RawQuery,
) -> Result<Response, Response> {
    let query_params = query::Query::from_params(query_params.unwrap_or_default().as_str());

    let query_params = QPV::new(query_params)
        .valid_pagination()
        .no_filter()
        .no_include()
        .only_one_required_sort(vec!["symbol", "-symbol"])
        .no_fields()
        .collect_query()?;

    let tokens = validators::validate_sqlx_response(
        db::all_token(
            &cp,
            query_params.page.unwrap().number,
            query_params.page.unwrap().size,
            query_params.sort,
        )
        .await,
    )?;

    validators::validate_vec_result(&tokens, query_params.page.unwrap().size, "Tokens")?;

    let total_count = validators::validate_sqlx_response(db::all_token_count(&cp).await)?;

    Ok(Json(utils::vec_to_jsonapi_document(
        tokens,
        total_count,
        query_params.page.unwrap(),
        "token",
    ))
    .into_response())
}

async fn get_balances(
    Extension(cp): Extension<PgPool>,
    RawQuery(query_params): RawQuery,
) -> Result<Response, Response> {
    let query_params = query::Query::from_params(query_params.unwrap_or_default().as_str());

    let query_params = QPV::new(query_params)
        .valid_pagination()
        .only_one_required_filter(vec!["holder.holder_addr", "token.contract_addr"])
        .no_fields()
        .only_one_required_sort(vec!["amount", "-amount"])
        .no_include()
        .collect_query()?;

    let balances = validators::validate_sqlx_response(
        db::all_balance_by_filter(
            &cp,
            query_params.filter.unwrap(),
            query_params.page.unwrap().number,
            query_params.page.unwrap().size,
            query_params.sort,
        )
        .await,
    )?;

    validators::validate_vec_result(&balances, query_params.page.unwrap().size, "Balances")?;

    let total_count = validators::validate_sqlx_response(db::all_token_count(&cp).await)?;

    Ok(Json(utils::vec_to_jsonapi_document(
        balances,
        total_count,
        query_params.page.unwrap(),
        "balance",
    ))
    .into_response())
}

async fn post_token(
    // TODO: evm connected creation
    Extension(cp): Extension<PgPool>,
    extract::Json(doc): Json<JsonApiDocument>,
) -> Result<Response, Response> {
    let data = validators::get_data_from_doc(doc)?;
    todo!()
    // match data.get_attribute("contract_addr") {
    //     Some(value) => match value.as_str() {
    //         None => Err(Json("Missing data").into_response()),
    //         Some(contract_addr) => {
    //             let token = validate_sqlx_response(add_token(&cp, todo!()).await)?;
    //             Ok((StatusCode::CREATED, Json(token.to_jsonapi_document())).into_response())
    //         }
    //     },
    //     None => Err(Json("Missing 'contract_addr' attribute").into_response()),
    // }
}
