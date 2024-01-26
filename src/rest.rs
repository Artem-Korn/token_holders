use axum::{extract::Path, http::StatusCode, routing::get, Extension, Json, Router};
use sqlx::PgPool;

use crate::db::{token_by_id, Token};

pub fn create_router(connection_pool: PgPool) -> Router {
    Router::new()
        .route("/:id", get(get_token))
        .layer(Extension(connection_pool))
}

async fn get_token(
    Extension(cp): Extension<PgPool>,
    Path(token_id): Path<i32>,
) -> Result<Json<Token>, StatusCode> {
    if let Ok(token) = token_by_id(&cp, token_id).await {
        Ok(Json(token))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
