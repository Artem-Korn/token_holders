use crate::{
    app_err_response,
    db::{self, Token},
    error::{AppError, AppErrorResponse},
    evm, utils,
    validators::QueryParamsValidator as QPV,
};
use axum::{
    extract::{self, RawQuery},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use jsonapi::{model::*, query};
use sqlx::PgPool;
use std::vec;
use tokio::sync::mpsc;

pub fn create_router(connection_pool: PgPool, tx: mpsc::Sender<Token>) -> Router {
    Router::new()
        .route(
            "/tokens",
            get(get_tokens).post(move |extension, body| post_token(extension, body, tx)),
        )
        .route("/balances", get(get_balances))
        .layer(Extension(connection_pool))
}

async fn get_tokens(
    Extension(cp): Extension<PgPool>,
    RawQuery(query_params): RawQuery,
) -> Result<Response, AppErrorResponse> {
    let query_params = query::Query::from_params(query_params.unwrap_or_default().as_str());

    let query_params = QPV::new(query_params)
        .valid_pagination()
        .only_one_sort(vec!["symbol", "-symbol"])
        .no_filter()
        .no_include()
        .no_fields()
        .collect_query()?;

    let tokens = db::all_token(
        &cp,
        query_params.page.unwrap().number,
        query_params.page.unwrap().size,
        query_params.sort,
    )
    .await?;

    let total_count = db::all_token_count(&cp).await?;

    Ok(Json(utils::vec_to_jsonapi_document(
        tokens,
        total_count,
        query_params.page.unwrap(),
        "token",
    )?)
    .into_response())
}

async fn get_balances(
    Extension(cp): Extension<PgPool>,
    RawQuery(query_params): RawQuery,
) -> Result<Response, AppErrorResponse> {
    let query_params = query::Query::from_params(query_params.unwrap_or_default().as_str());

    let query_params = QPV::new(query_params)
        .valid_pagination()
        .only_one_filter(vec!["holder.holder_addr", "token.contract_addr"])
        .no_fields()
        .only_one_sort(vec!["amount", "-amount"])
        .no_include()
        .collect_query()?;

    let filter = query_params.filter.clone().unwrap();

    let balances = db::all_balance_by_filter(
        &cp,
        query_params.filter.unwrap(),
        query_params.page.unwrap().number,
        query_params.page.unwrap().size,
        query_params.sort,
    )
    .await?;

    let total_count = db::all_balance_by_filter_count(&cp, filter).await?;

    Ok(Json(utils::vec_to_jsonapi_document(
        balances,
        total_count,
        query_params.page.unwrap(),
        "balance",
    )?)
    .into_response())
}

async fn post_token(
    Extension(cp): Extension<PgPool>,
    extract::Json(doc): Json<JsonApiDocument>,
    tx: mpsc::Sender<Token>,
) -> Result<Response, AppErrorResponse> {
    let data = utils::get_data_from_doc(doc)?;

    match data.get_attribute("contract_addr") {
        Some(value) => match value.as_str() {
            None => Err(app_err_response!(StatusCode::BAD_REQUEST, "Missing data")),
            Some(contract_addr) => {
                if let Ok(token) = evm::add_token_by_contract(&cp, contract_addr).await {
                    let send = tx.send(token.clone()).await;
                    match send {
                        Ok(_) => Ok((StatusCode::CREATED, Json(token.to_jsonapi_document()))
                            .into_response()),
                        Err(err) => Err(app_err_response!(StatusCode::BAD_REQUEST, err)),
                    }
                } else {
                    Err(app_err_response!(
                        StatusCode::BAD_REQUEST,
                        "Add token by contract error"
                    ))
                }
            }
        },
        None => Err(app_err_response!(
            StatusCode::BAD_REQUEST,
            "Missing 'contract_addr' attribute"
        )),
    }
}
