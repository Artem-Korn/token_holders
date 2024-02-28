use crate::error::{AppError, AppErrorResponse};
use axum::http::StatusCode;
use jsonapi::query::Query;

pub struct QueryParamsValidator {
    query_params: Query,
    errors_vec: Vec<AppError>,
    cur_param_name: String,
}

impl QueryParamsValidator {
    pub fn new(query_params: Query) -> Self {
        Self {
            query_params,
            errors_vec: Vec::<AppError>::new(),
            cur_param_name: String::new(),
        }
    }

    fn add_error(mut self, message: &str) {
        self.errors_vec.push(AppError::new(
            StatusCode::BAD_REQUEST,
            message,
            Some(message),
            Some(&self.cur_param_name),
        ));
    }

    pub fn collect_query(self) -> Result<Query, AppErrorResponse> {
        if self.errors_vec.is_empty() {
            Ok(self.query_params)
        } else {
            Err(AppErrorResponse::new(
                StatusCode::BAD_REQUEST,
                self.errors_vec,
            ))
        }
    }

    pub fn valid_pagination(mut self) -> Self {
        let page = self.query_params.page.unwrap();

        let mut validate = |attribute, name| {
            if attribute < 0 {
                let message = format!("pagination attribute {name} less then 0");

                self.errors_vec.push(AppError::new(
                    StatusCode::BAD_REQUEST,
                    &message,
                    Some(&message),
                    Some(&format!("page/{name}")),
                ));
            }
        };

        validate(page.size, "size");
        validate(page.number, "number");

        self
    }

    fn valid_vector(vector: Vec<String>, valid_vector: &Vec<&str>) -> Vec<String> {
        vector
            .into_iter()
            .filter(|value| valid_vector.contains(&value.as_str()))
            .collect()
    }

    fn unwrap<T: Default>(mut self, collection: Option<T>) -> T {
        match collection {
            Some(collection) => collection,
            None => {
                let message = format!("'{}' attribute is missing", self.cur_param_name);
                self.add_error(&message);

                Default::default()
            }
        }
    }

    fn no<T>(mut self, collection: Option<T>) {
        if collection.is_some() {
            let message = format!("'{}' attribute cannot be set", self.cur_param_name);
            self.add_error(&message);
        }
    }

    fn only(mut self, vector_len: usize, valid_vector_len: usize, required: &Vec<&str>) {
        if vector_len != valid_vector_len {
            let message = format!(
                "'{}' attribute cannot have optional values. Required values: {}",
                self.cur_param_name,
                required.join(", ")
            );
            self.add_error(&message);
        }
    }

    fn one(mut self, vector_len: usize, valid_vector_len: usize, required: &Vec<&str>) {
        if vector_len != valid_vector_len {
            let message = format!(
                "'{}' attribute cannot have more then one of required values. Required values: {}",
                self.cur_param_name,
                required.join(", ")
            );
            self.add_error(&message);
        }
    }

    fn only_one(mut self, vector: Vec<String>, required_vector: Vec<&str>) {
        let vector_len = vector.len();
        let valid_vector = Self::valid_vector(vector, &required_vector);
        let valid_vector_len = valid_vector.len();

        self.only(vector_len, valid_vector_len, &required_vector);
        self.one(vector_len, valid_vector_len, &required_vector);
    }

    pub fn no_filter(mut self) -> Self {
        self.cur_param_name = "filter".to_string();
        self.no(self.query_params.filter);

        self
    }

    pub fn no_include(mut self) -> Self {
        self.cur_param_name = "include".to_string();
        self.no(self.query_params.include);

        self
    }

    pub fn no_fields(mut self) -> Self {
        self.cur_param_name = "fields".to_string();
        self.no(self.query_params.fields);

        self
    }

    pub fn only_one_sort(mut self, required_sort: Vec<&str>) -> Self {
        self.cur_param_name = "sort".to_string();

        let sort_params = self.unwrap(self.query_params.sort.clone());

        self.only_one(sort_params, required_sort);

        self
    }

    pub fn only_one_filter(mut self, required_filter: Vec<&str>) -> Self {
        self.cur_param_name = "filter".to_string();

        let filter_keys: Vec<_> = self
            .unwrap(self.query_params.filter.clone())
            .keys()
            .cloned()
            .collect();

        self.only_one(filter_keys, required_filter);

        self
    }
}
