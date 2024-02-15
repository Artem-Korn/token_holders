use jsonapi::{model::*, query::PageParams};
use serde_json::json;
use std::collections::HashSet;

//TODO: Make it better
fn create_meta_hashmap(
    total_count: i64,
    page: PageParams,
    route: &str,
) -> Option<HashMap<String, JsonApiValue>> {
    let mut links: HashMap<String, JsonApiValue> = HashMap::new();

    let link_1 = format!(
        "{}:{}/{}?page[number]=",
        std::env::var("SERVICE_IP").unwrap(),
        std::env::var("SERVICE_PORT").unwrap(),
        route
    );
    let link_2 = ",page[size]=";

    let last = total_count / page.size - 1;

    if page.number != 0 {
        links.insert(
            "first".to_string(),
            json!(format!("{}{}{}{}", link_1, 0, link_2, page.size)),
        );
    }

    if page.number - 1 >= 0 {
        links.insert(
            "prev".to_string(),
            json!(format!(
                "{}{}{}{}",
                link_1,
                page.number - 1,
                link_2,
                page.size
            )),
        );
    }

    if page.number + 1 <= last {
        links.insert(
            "next".to_string(),
            json!(format!(
                "{}{}{}{}",
                link_1,
                page.number + 1,
                link_2,
                page.size
            )),
        );
    }

    if page.number != last {
        links.insert(
            "last".to_string(),
            json!(format!("{}{}{}{}", link_1, page.number, link_2, page.size)),
        );
    }

    Some(links)
}

pub fn vec_to_jsonapi_document<T: JsonApiModel>(
    objects: Vec<T>,
    total_count: i64,
    page: PageParams,
    route: &str,
) -> JsonApiDocument {
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

    let mut meta: HashMap<String, JsonApiValue> = HashMap::new();
    meta.insert("total_count".to_string(), json!(total_count));
    meta.insert("page_number".to_string(), json!(page.number));
    meta.insert("page_size".to_string(), json!(page.size));

    JsonApiDocument::Data(DocumentData {
        data: Some(PrimaryData::Multiple(resources)),
        included,
        meta: Some(meta),
        links: create_meta_hashmap(total_count, page, route),
        ..Default::default()
    })
}
