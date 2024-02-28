use crate::{
    app_err_response,
    error::{AppError, AppErrorResponse},
};
use anyhow::Result;
use axum::http::StatusCode;
use jsonapi::{model::*, query::PageParams};
use serde_json::json;
use std::collections::HashSet;

fn create_link(url: &str, page_number: i64, page_size: i64) -> String {
    format!(
        "{}?page[number]={},page[size]={}",
        url, page_number, page_size
    )
}

fn create_links_hashmap(
    total_count: i64,
    page: PageParams,
    url: &str,
) -> Option<HashMap<String, JsonApiValue>> {
    let mut links: HashMap<String, JsonApiValue> = HashMap::new();

    let last = if page.size != 0 {
        total_count / page.size - 1
    } else {
        0
    };

    let create_link = |page_number: i64| json!(create_link(url, page_number, page.size));

    if page.number > 0 {
        links.insert("first".to_string(), json!(create_link(0)));
        links.insert("prev".to_string(), json!(create_link(page.number - 1)));
    }

    if page.number < last {
        links.insert("next".to_string(), json!(create_link(page.number + 1)));
        links.insert("last".to_string(), json!(create_link(last)));
    }

    if links.is_empty() {
        None
    } else {
        Some(links)
    }
}

/// Create jsonapi document from vector with meta, links and included
pub fn vec_to_jsonapi_document<T: JsonApiModel>(
    objects: Vec<T>,
    total_count: i64,
    page: PageParams,
    route: &str,
) -> Result<JsonApiDocument, AppErrorResponse> {
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

    let url = format!(
        "{}:{}/{}",
        std::env::var("SERVICE_IP")
            .or_else(|err| Err(app_err_response!(StatusCode::INTERNAL_SERVER_ERROR, err)))?,
        std::env::var("SERVICE_PORT")
            .or_else(|err| Err(app_err_response!(StatusCode::INTERNAL_SERVER_ERROR, err)))?,
        route
    );

    let mut meta: HashMap<String, JsonApiValue> = HashMap::new();
    meta.insert("total_count".to_string(), json!(total_count));
    meta.insert("page_number".to_string(), json!(page.number));
    meta.insert("page_size".to_string(), json!(page.size));

    Ok(JsonApiDocument::Data(DocumentData {
        data: Some(PrimaryData::Multiple(resources)),
        included,
        meta: Some(meta),
        links: create_links_hashmap(total_count, page, &url),
        ..Default::default()
    }))
}

pub fn get_data_from_doc(doc: JsonApiDocument) -> Result<Resource, AppErrorResponse> {
    match doc.validate() {
        Some(error) => {
            if error.contains(&DocumentValidationError::IncludedWithoutData) {
                Err(app_err_response!(
                    StatusCode::BAD_REQUEST,
                    "Included Without Data"
                ))
            } else {
                Err(app_err_response!(
                    StatusCode::BAD_REQUEST,
                    "Missing Content"
                ))
            }
        }
        None => match doc {
            JsonApiDocument::Error(_) => Err(app_err_response!(
                StatusCode::BAD_REQUEST,
                "Contains Error Data"
            )),
            JsonApiDocument::Data(doc) => match doc.data {
                Some(PrimaryData::Single(data)) => Ok(*data),
                Some(_) => Err(app_err_response!(
                    StatusCode::BAD_REQUEST,
                    "Too Many Resources"
                )),
                None => Err(app_err_response!(StatusCode::BAD_REQUEST, "Missing Data")),
            },
        },
    }
}
