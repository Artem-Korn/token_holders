use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jsonapi::{
    model::{DocumentError, ErrorSource, JsonApiDocument, JsonApiError},
    query::PageParams,
};
use std::collections::HashMap;

enum QueryParamsError {
    Missing,
    Invalid,
}

impl QueryParamsError {
    fn get_title(&self) -> Option<String> {
        match self {
            QueryParamsError::Missing => Some(String::from("Missing Required Attribute")),
            QueryParamsError::Invalid => Some(String::from("Invalid Attribute Value")),
        }
    }
}

pub struct QueryParamsValidator {
    errors: Vec<JsonApiError>,
}

impl QueryParamsValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    fn unbracket(param_name: &str) -> Option<(String, String)> {
        if let Some(open_bracket) = param_name.find('[') {
            if let Some(close_bracket) = param_name.rfind(']') {
                if open_bracket < close_bracket {
                    return Some((
                        param_name[..open_bracket].to_string(),
                        param_name[open_bracket + 1..close_bracket].to_string(),
                    ));
                }
            }
        }
        None
    }

    fn name_to_pointer(param_name: &str) -> String {
        match Self::unbracket(param_name) {
            Some((param, name)) => format!("/data/attributes/{}/{}", param, name),
            None => format!("/data/attributes/{}", param_name),
        }
    }

    fn add_error(&mut self, param_name: &str, error: QueryParamsError, detail: Option<String>) {
        self.errors.push(JsonApiError {
            status: Some(String::from("400")),
            title: error.get_title(),
            detail,
            source: Some(ErrorSource {
                pointer: Some(Self::name_to_pointer(param_name)),
                parameter: Some(String::from(param_name)),
            }),
            ..Default::default()
        })
    }

    fn add_error_if(
        &mut self,
        condition: bool,
        param_name: &str,
        error: QueryParamsError,
        detail: Option<String>,
    ) {
        if condition {
            self.add_error(param_name, error, detail);
        }
    }

    fn verify_existence<T>(&mut self, param: Option<T>, required: bool, param_name: &str) -> T
    where
        T: Default,
    {
        match param {
            Some(param) => param,
            None => {
                self.add_error_if(
                    required,
                    param_name,
                    QueryParamsError::Missing,
                    Some(format!("Missing required attribute \'{}\'.", param_name)),
                );
                Default::default()
            }
        }
    }

    pub fn check_for_errors(&mut self) -> Result<(), Response> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            let body = Json(JsonApiDocument::Error(DocumentError {
                errors: self.errors.clone(),
                ..Default::default()
            }));
            self.errors.clear();

            Err((StatusCode::BAD_REQUEST, body).into_response())
        }
    }

    pub fn pagination_parse(&mut self, page: Option<PageParams>) -> Option<(i64, i64)> {
        match page {
            Some(page) if page.number >= 0 && page.size >= 0 => {
                Some((page.number * page.size, page.size))
            }
            Some(page) => {
                self.add_error_if(
                    page.number < 0,
                    "page[number]",
                    QueryParamsError::Invalid,
                    Some(String::from("Page number must be positive.")),
                );
                self.add_error_if(
                    page.size < 0,
                    "page[size]",
                    QueryParamsError::Invalid,
                    Some(String::from("Page size must be positive.")),
                );
                None
            }
            None => None,
        }
    }

    pub fn vec_stores_param(&mut self, vector: &Vec<String>, param_full: &str) -> Option<String> {
        let (attribute, param) = Self::unbracket(param_full).unwrap();

        if vector.contains(&param) {
            Some(param)
        } else {
            self.add_error(
                param_full,
                QueryParamsError::Missing,
                Some(format!(
                    "No required \'{}\' found among the attributes \'{}\'.",
                    param, attribute
                )),
            );
            None
        }
    }

    pub fn hashmap_stores_param(
        &mut self,
        hashmap: &HashMap<String, Vec<String>>,
        param_full: &str,
    ) -> Option<Vec<String>> {
        let (attribute, param) = Self::unbracket(param_full).unwrap();

        match hashmap.get(&param) {
            Some(value) => Some(value.to_vec()),
            None => {
                self.add_error(
                    param_full,
                    QueryParamsError::Missing,
                    Some(format!(
                        "No required \'{}\' found among the attributes \'{}\'.",
                        param, attribute
                    )),
                );
                None
            }
        }
    }
}

fn create_error_doc(status: &str, title: &str, detail: String) -> Json<JsonApiDocument> {
    Json(JsonApiDocument::Error(DocumentError {
        errors: vec![JsonApiError {
            status: Some(String::from(status)),
            title: Some(String::from(title)),
            detail: Some(detail),
            ..Default::default()
        }],
        ..Default::default()
    }))
}

fn sqlx_error_parse(error: sqlx::Error) -> Response {
    match error {
        sqlx::Error::Database(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JsonApiDocument::Error(DocumentError {
                errors: vec![JsonApiError {
                    status: Some(String::from("500")),
                    code: Some(error.code().unwrap().to_string()),
                    title: Some(String::from("Database Error")),
                    detail: Some(error.message().to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            })),
        )
            .into_response(),

        sqlx::Error::RowNotFound => (
            StatusCode::NOT_FOUND,
            create_error_doc("404", "Row Not Found", error.to_string()),
        )
            .into_response(),

        sqlx::Error::TypeNotFound { type_name } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            create_error_doc(
                "422",
                "Type Not Found",
                format!("Type \'{}\' in query doesn't exist.", type_name),
            ),
        )
            .into_response(),

        sqlx::Error::ColumnIndexOutOfBounds { index, len } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            create_error_doc(
                "422",
                "Column Index Out Of Bounds",
                format!(
                    "Column index ({}) is out of bounds (length: {}).",
                    index, len
                ),
            ),
        )
            .into_response(),

        sqlx::Error::ColumnNotFound(col_name) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            create_error_doc(
                "422",
                "Column Not Found",
                format!("No column found for the given name \'{}\'.", col_name),
            ),
        )
            .into_response(),

        sqlx::Error::ColumnDecode { index, source } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            create_error_doc("422", "Column Decode Error", source.to_string()),
        )
            .into_response(),

        sqlx::Error::Decode(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            create_error_doc("422", "Decode Error", error.to_string()),
        )
            .into_response(),

        sqlx::Error::PoolTimedOut => (
            StatusCode::SERVICE_UNAVAILABLE,
            create_error_doc("503", "Pool Timed Out", error.to_string()),
        )
            .into_response(),

        sqlx::Error::PoolClosed => (
            StatusCode::SERVICE_UNAVAILABLE,
            create_error_doc("503", "Pool Closed", error.to_string()),
        )
            .into_response(),

        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JsonApiDocument::Error(DocumentError {
                errors: vec![JsonApiError {
                    status: Some(String::from("500")),
                    detail: Some(error.to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            })),
        )
            .into_response(),
    }
}

pub fn validate_sqlx_response<T>(response: Result<T, sqlx::Error>) -> Result<T, Response> {
    match response {
        Ok(value) => Ok(value),
        Err(error) => Err(sqlx_error_parse(error)),
    }
}
