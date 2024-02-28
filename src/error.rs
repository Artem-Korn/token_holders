use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jsonapi::api::{DocumentError, ErrorSource, JsonApiDocument, JsonApiError};

#[macro_export]
macro_rules! app_err_response {
    ($code:expr, $err:expr) => {
        AppErrorResponse::new($code, vec![AppError::new($code, $err, None, None)])
    };
}

pub struct AppError {
    body: JsonApiError,
}

impl AppError {
    pub fn new(
        code: StatusCode,
        error: impl ToString,
        detail: Option<&str>,
        param_name: Option<&str>,
    ) -> Self {
        tracing::error!("{}", error.to_string());

        Self {
            body: JsonApiError {
                status: Some(code.as_u16().to_string()),
                title: code.canonical_reason().map(|s| s.to_string()),
                detail: detail.map(|s| s.to_string()),
                source: Self::get_source(param_name),
                ..Default::default()
            },
        }
    }

    fn get_source(param_name: Option<&str>) -> Option<ErrorSource> {
        match param_name {
            Some(param_name) => Some(ErrorSource {
                pointer: Some(format!("/data/attributes/{}", param_name)),
                parameter: Some(param_name.to_string()),
            }),
            None => None,
        }
    }
}

pub struct AppErrorResponse {
    code: StatusCode,
    body: Json<JsonApiDocument>,
}

impl AppErrorResponse {
    pub fn new(code: StatusCode, errors: Vec<AppError>) -> Self {
        Self {
            code,
            body: Json(JsonApiDocument::Error(DocumentError {
                errors: errors.iter().map(|err| err.body.clone()).collect(),
                ..Default::default()
            })),
        }
    }
}

impl IntoResponse for AppErrorResponse {
    fn into_response(self) -> Response {
        (self.code, self.body).into_response()
    }
}

impl From<sqlx::Error> for AppErrorResponse {
    fn from(value: sqlx::Error) -> Self {
        app_err_response!(StatusCode::INTERNAL_SERVER_ERROR, value)
    }
}
