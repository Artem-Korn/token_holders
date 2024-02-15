use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jsonapi::{model::*, query::Query};

enum ErrorDetails<'a> {
    // NotAllReqValues(Vec<&'a str>),
    CannotBeSet,
    CannotHaveOptionalField(Vec<&'a str>),
    LessThanZero,
    NotOneValue,
    OnlyOneReqField(Vec<&'a str>),
    OnlyOneReqValue(Vec<&'a str>),
    ReqAttribute,
}

impl ErrorDetails<'_> {
    fn to_string(&self) -> String {
        match self {
            // ErrorDetails::NotAllReqValues(required_values) => format!(
            //         "attribute does not have all required values: {}. It can also be the result of duplication of the attribute name.",
            //         required_values.join(", ")
            //     ),
            ErrorDetails::CannotBeSet => "attribute cannot be set.".to_string(),
            ErrorDetails::CannotHaveOptionalField(required_fields) => format!(
                    "attribute cannot have optional fields names, only required: {}.",
                    required_fields.join(", ")
                ),
            ErrorDetails::LessThanZero => "attribute cannot be less than zero.".to_string(),
            ErrorDetails::NotOneValue => "attribute cannot have more than one value or no value at all.".to_string(),
            ErrorDetails::OnlyOneReqField(required_fields) => format!(
                    "attribute must contain only one of the required field names at a time. Required field names: {}.",
                    required_fields.join(", ")
                ),
            ErrorDetails::OnlyOneReqValue(required_values) => format!(
                    "attribute must contain only one of the required values at a time. Required values: {}.",
                    required_values.join(", ")
                ),
            ErrorDetails::ReqAttribute => "attribute is required. It can also be the result of duplication of the attribute name.".to_string(),
        }
    }
}

enum QueryParamsError<'a> {
    Missing(&'a str, ErrorDetails<'a>),
    Invalid(&'a str, ErrorDetails<'a>),
}

impl QueryParamsError<'_> {
    fn get_title(&self) -> Option<String> {
        match self {
            QueryParamsError::Missing(_, _) => Some(String::from("Missing Required Attribute")),
            QueryParamsError::Invalid(_, _) => Some(String::from("Invalid Attribute Value")),
        }
    }

    fn get_source(&self) -> Option<ErrorSource> {
        match self {
            QueryParamsError::Missing(param, _) | QueryParamsError::Invalid(param, _) => {
                Some(ErrorSource {
                    pointer: Some(format!("/data/attributes/{}", param)),
                    parameter: Some(param.to_string()),
                })
            }
        }
    }

    fn get_details(&self) -> Option<String> {
        match self {
            QueryParamsError::Missing(param, details)
            | QueryParamsError::Invalid(param, details) => {
                Some(format!("\'{}\' {}", param, details.to_string()))
            }
        }
    }
}

pub struct QueryParamsValidator {
    query_params: Query,
    errors_vec: Vec<JsonApiError>,
}

// TODO: Change with macro
impl QueryParamsValidator {
    pub fn new(query_params: Query) -> Self {
        return Self {
            query_params,
            errors_vec: Vec::new(),
        };
    }

    fn add_error(&mut self, error: QueryParamsError) {
        self.errors_vec.push(JsonApiError {
            status: Some(String::from("400")),
            title: error.get_title(),
            detail: error.get_details(),
            source: error.get_source(),
            ..Default::default()
        })
    }

    pub fn valid_pagination(mut self) -> Self {
        let page = self.query_params.page.unwrap();

        let mut validate = |attribute, error_message| {
            if attribute < 0 {
                self.add_error(QueryParamsError::Invalid(
                    &format!("page/{}", error_message),
                    ErrorDetails::LessThanZero,
                ));
            }
        };

        validate(page.size, "size");
        validate(page.number, "number");

        self
    }

    pub fn only_one_required_filter(mut self, required_filters: Vec<&str>) -> Self {
        let filter_params = self.query_params.filter.clone();

        match filter_params {
            Some(filter_params) => {
                let filter_len = filter_params.len();

                let provided_params: Vec<_> = filter_params
                    .into_iter()
                    .filter(|(key, _)| required_filters.contains(&key.as_str()))
                    .collect();

                if provided_params.len() != filter_len {
                    self.add_error(QueryParamsError::Invalid(
                        "filter",
                        ErrorDetails::CannotHaveOptionalField(required_filters),
                    ))
                } else if provided_params.len() != 1 {
                    self.add_error(QueryParamsError::Invalid(
                        "filter",
                        ErrorDetails::OnlyOneReqField(required_filters),
                    ));
                } else {
                    let (name, value) = provided_params.first().unwrap();

                    if value.len() != 1 {
                        self.add_error(QueryParamsError::Invalid(
                            &format!("filter/{}", name),
                            ErrorDetails::NotOneValue,
                        ));
                    }
                }
            }
            None => self.add_error(QueryParamsError::Missing(
                "filter",
                ErrorDetails::ReqAttribute,
            )),
        }

        self
    }

    // pub fn only_required_include(mut self, required_include: Vec<&str>) -> Self {
    //     let include_params = self.query_params.include.clone();

    //     match include_params {
    //         Some(include_params) => {
    //             let include_len = include_params.len();

    //             let provided_params: Vec<_> = include_params
    //                 .into_iter()
    //                 .filter(|value| required_include.contains(&value.as_str()))
    //                 .collect();

    //             if provided_params.len() != include_len {
    //                 self.add_error(QueryParamsError::Invalid(
    //                     "include",
    //                     ErrorDetails::CannotHaveOptionalField(required_include),
    //                 ));
    //             } else if provided_params.len() != required_include.len() {
    //                 self.add_error(QueryParamsError::Invalid(
    //                     "include",
    //                     ErrorDetails::NotAllReqValues(required_include),
    //                 ));
    //             }
    //         }
    //         None => self.add_error(QueryParamsError::Missing(
    //             "include",
    //             ErrorDetails::ReqAttribute,
    //         )),
    //     }

    //     self
    // }

    pub fn only_one_required_sort(mut self, required_sort: Vec<&str>) -> Self {
        let sort_params = self.query_params.sort.clone();

        match sort_params {
            Some(sort_params) => {
                let sort_len = sort_params.len();

                let provided_params: Vec<_> = sort_params
                    .into_iter()
                    .filter(|value| required_sort.contains(&value.as_str()))
                    .collect();

                if provided_params.len() != sort_len {
                    self.add_error(QueryParamsError::Invalid(
                        "sort",
                        ErrorDetails::CannotHaveOptionalField(required_sort),
                    ));
                } else if provided_params.len() != 1 {
                    self.add_error(QueryParamsError::Invalid(
                        "sort",
                        ErrorDetails::OnlyOneReqValue(required_sort),
                    ));
                }
            }
            None => self.add_error(QueryParamsError::Missing(
                "sort",
                ErrorDetails::ReqAttribute,
            )),
        }

        self
    }

    pub fn no_filter(mut self) -> Self {
        if !self.query_params.filter.is_none() {
            self.add_error(QueryParamsError::Invalid(
                "filter",
                ErrorDetails::CannotBeSet,
            ))
        }

        self
    }

    pub fn no_include(mut self) -> Self {
        if !self.query_params.include.is_none() {
            self.add_error(QueryParamsError::Invalid(
                "include",
                ErrorDetails::CannotBeSet,
            ))
        }

        self
    }

    pub fn no_fields(mut self) -> Self {
        if !self.query_params.fields.as_ref().unwrap().is_empty() {
            self.add_error(QueryParamsError::Invalid(
                "fields",
                ErrorDetails::CannotBeSet,
            ))
        }

        self
    }

    // pub fn print(self) -> Self {
    //     println!("{:?}", self.query_params);
    //     self
    // }

    pub fn collect_query(self) -> Result<Query, Response> {
        if self.errors_vec.is_empty() {
            Ok(self.query_params)
        } else {
            let body = Json(JsonApiDocument::Error(DocumentError {
                errors: self.errors_vec,
                ..Default::default()
            }));
            Err((StatusCode::BAD_REQUEST, body).into_response())
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

// TODO: Change with macro
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
                format!("Type '{}' in query doesn't exist.", type_name),
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
                format!("No column found for the given name '{}'.", col_name),
            ),
        )
            .into_response(),

        sqlx::Error::ColumnDecode { index: _, source } => (
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

pub fn validate_vec_result<T>(
    result: &Vec<T>,
    expected_size: i64,
    name: &str,
) -> Result<(), Response> {
    if result.len() == 0 && expected_size != 0 {
        Err((
            StatusCode::NOT_FOUND,
            Json(JsonApiDocument::Error(DocumentError {
                errors: vec![JsonApiError {
                    status: Some(String::from("404")),
                    title: Some(format!("{name} Not Found")),
                    ..Default::default()
                }],
                ..Default::default()
            })),
        )
            .into_response())
    } else {
        Ok(())
    }
}

pub fn get_data_from_doc(doc: JsonApiDocument) -> Result<Resource, Response> {
    match doc.validate() {
        Some(error) => {
            if error.contains(&DocumentValidationError::IncludedWithoutData) {
                Err((
                    StatusCode::BAD_REQUEST,
                    Json(JsonApiDocument::Error(DocumentError {
                        errors: vec![JsonApiError {
                            status: Some(String::from("400")),
                            title: Some(format!("Included Without Data")),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                )
                    .into_response())
            } else {
                Err((
                    StatusCode::BAD_REQUEST,
                    Json(JsonApiDocument::Error(DocumentError {
                        errors: vec![JsonApiError {
                            status: Some(String::from("400")),
                            title: Some(format!("Missing Content")),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                )
                    .into_response())
            }
        }
        None => match doc {
            JsonApiDocument::Error(_) => Err((
                StatusCode::BAD_REQUEST,
                Json(JsonApiDocument::Error(DocumentError {
                    errors: vec![JsonApiError {
                        status: Some(String::from("400")),
                        title: Some(format!("Contains Error Data")),
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            )
                .into_response()),
            JsonApiDocument::Data(docdata) => match docdata.data {
                Some(PrimaryData::Single(data)) => Ok(*data),
                Some(_) => Err((
                    StatusCode::BAD_REQUEST,
                    Json(JsonApiDocument::Error(DocumentError {
                        errors: vec![JsonApiError {
                            status: Some(String::from("400")),
                            title: Some(format!("Too Many Resources")),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                )
                    .into_response()),
                None => Err((
                    StatusCode::BAD_REQUEST,
                    Json(JsonApiDocument::Error(DocumentError {
                        errors: vec![JsonApiError {
                            status: Some(String::from("400")),
                            title: Some(format!("Missing Data")),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                )
                    .into_response()),
            },
        },
    }
}
