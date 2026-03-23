use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::route::RouteTable;

/// Configuration for the generated OpenAPI document.
pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
    pub description: Option<String>,
}

impl OpenApiInfo {
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            description: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Convert a harrow path pattern to OpenAPI path format.
/// `:id` -> `{id}`, `*path` -> `{path}`
fn to_openapi_path(pattern: &str) -> String {
    pattern
        .split('/')
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':') {
                format!("{{{name}}}")
            } else if let Some(name) = seg.strip_prefix('*') {
                format!("{{{name}}}")
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Extract path parameter names and their types from a harrow pattern.
fn extract_path_params(pattern: &str) -> Vec<Value> {
    pattern
        .split('/')
        .filter_map(|seg| {
            let name = seg.strip_prefix(':').or_else(|| seg.strip_prefix('*'))?;
            Some(json!({
                "name": name,
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            }))
        })
        .collect()
}

impl RouteTable {
    /// Generate an OpenAPI 3.0.3 JSON document from the route table.
    pub fn to_openapi_json(&self, info: &OpenApiInfo) -> String {
        let mut info_obj = json!({
            "title": info.title,
            "version": info.version,
        });
        if let Some(desc) = &info.description {
            info_obj["description"] = Value::String(desc.clone());
        }

        // Group routes by pattern, preserving insertion order via BTreeMap
        let mut paths: BTreeMap<String, Map<String, Value>> = BTreeMap::new();

        for route in self.iter() {
            let pattern = route.pattern.as_str();
            let openapi_path = to_openapi_path(pattern);
            let method = route.method.as_str().to_lowercase();

            let mut operation = Map::new();

            if let Some(name) = &route.metadata.name {
                operation.insert("operationId".to_string(), Value::String(name.clone()));
            }

            if !route.metadata.tags.is_empty() {
                let tags: Vec<Value> = route
                    .metadata
                    .tags
                    .iter()
                    .map(|t| Value::String(t.clone()))
                    .collect();
                operation.insert("tags".to_string(), Value::Array(tags));
            }

            if route.metadata.deprecated {
                operation.insert("deprecated".to_string(), Value::Bool(true));
            }

            let params = extract_path_params(pattern);
            if !params.is_empty() {
                operation.insert("parameters".to_string(), Value::Array(params));
            }

            operation.insert(
                "responses".to_string(),
                json!({
                    "200": {
                        "description": "Successful response"
                    }
                }),
            );

            let path_item = paths.entry(openapi_path).or_default();
            path_item.insert(method, Value::Object(operation));
        }

        // Convert BTreeMap to serde_json::Value
        let paths_value: Map<String, Value> = paths
            .into_iter()
            .map(|(k, v)| (k, Value::Object(v)))
            .collect();

        let doc = json!({
            "openapi": "3.0.3",
            "info": info_obj,
            "paths": paths_value,
        });

        serde_json::to_string_pretty(&doc).expect("OpenAPI JSON serialization should not fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler;
    use crate::path::PathPattern;
    use crate::request::Request;
    use crate::response::Response;
    use crate::route::{Route, RouteMetadata};
    use http::Method;

    async fn dummy(_req: Request) -> Response {
        Response::text("ok")
    }

    fn make_route(method: Method, pattern: &str) -> Route {
        Route {
            method,
            pattern: PathPattern::parse(pattern),
            handler: handler::wrap(dummy),
            metadata: RouteMetadata::default(),
            middleware: Vec::new(),
        }
    }

    fn make_route_with_metadata(
        method: Method,
        pattern: &str,
        name: Option<&str>,
        tags: &[&str],
        deprecated: bool,
    ) -> Route {
        Route {
            method,
            pattern: PathPattern::parse(pattern),
            handler: handler::wrap(dummy),
            metadata: RouteMetadata {
                name: name.map(|s| s.to_string()),
                tags: tags.iter().map(|s| s.to_string()).collect(),
                deprecated,
                custom: Default::default(),
            },
            middleware: Vec::new(),
        }
    }

    #[test]
    fn empty_route_table_produces_valid_openapi() {
        let table = RouteTable::new();
        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(doc["openapi"], "3.0.3");
        assert_eq!(doc["info"]["title"], "Test");
        assert_eq!(doc["info"]["version"], "1.0.0");
        assert_eq!(doc["paths"], json!({}));
    }

    #[test]
    fn info_description_is_included() {
        let table = RouteTable::new();
        let info = OpenApiInfo::new("My API", "2.0.0").description("A cool API");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(doc["info"]["description"], "A cool API");
    }

    #[test]
    fn simple_routes_generate_paths() {
        let mut table = RouteTable::new();
        table.push(make_route(Method::GET, "/health"));
        table.push(make_route(Method::GET, "/users"));
        table.push(make_route(Method::POST, "/users"));

        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        assert!(doc["paths"]["/health"]["get"].is_object());
        assert!(doc["paths"]["/users"]["get"].is_object());
        assert!(doc["paths"]["/users"]["post"].is_object());
    }

    #[test]
    fn path_params_converted_to_openapi_format() {
        let mut table = RouteTable::new();
        table.push(make_route(Method::GET, "/users/:id"));

        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        let path = &doc["paths"]["/users/{id}"];
        assert!(path["get"].is_object());

        let params = &path["get"]["parameters"];
        assert_eq!(params[0]["name"], "id");
        assert_eq!(params[0]["in"], "path");
        assert_eq!(params[0]["required"], true);
    }

    #[test]
    fn glob_params_converted_to_openapi_format() {
        let mut table = RouteTable::new();
        table.push(make_route(Method::GET, "/files/*path"));

        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        let path = &doc["paths"]["/files/{path}"];
        assert!(path["get"].is_object());

        let params = &path["get"]["parameters"];
        assert_eq!(params[0]["name"], "path");
    }

    #[test]
    fn metadata_maps_to_operation_fields() {
        let mut table = RouteTable::new();
        table.push(make_route_with_metadata(
            Method::GET,
            "/users",
            Some("listUsers"),
            &["users", "admin"],
            false,
        ));
        table.push(make_route_with_metadata(
            Method::DELETE,
            "/users/:id",
            Some("deleteUser"),
            &["users"],
            true,
        ));

        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        let list = &doc["paths"]["/users"]["get"];
        assert_eq!(list["operationId"], "listUsers");
        assert_eq!(list["tags"], json!(["users", "admin"]));
        assert!(list.get("deprecated").is_none());

        let delete = &doc["paths"]["/users/{id}"]["delete"];
        assert_eq!(delete["operationId"], "deleteUser");
        assert_eq!(delete["tags"], json!(["users"]));
        assert_eq!(delete["deprecated"], true);
    }

    #[test]
    fn multiple_methods_on_same_path_grouped() {
        let mut table = RouteTable::new();
        table.push(make_route(Method::GET, "/users"));
        table.push(make_route(Method::POST, "/users"));
        table.push(make_route(Method::DELETE, "/users"));

        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        let users = &doc["paths"]["/users"];
        assert!(users["get"].is_object());
        assert!(users["post"].is_object());
        assert!(users["delete"].is_object());
    }

    #[test]
    fn multi_param_path() {
        let mut table = RouteTable::new();
        table.push(make_route(Method::GET, "/orgs/:org/repos/:repo"));

        let info = OpenApiInfo::new("Test", "1.0.0");
        let json_str = table.to_openapi_json(&info);
        let doc: Value = serde_json::from_str(&json_str).unwrap();

        let path = &doc["paths"]["/orgs/{org}/repos/{repo}"];
        let params = &path["get"]["parameters"];
        assert_eq!(params.as_array().unwrap().len(), 2);
        assert_eq!(params[0]["name"], "org");
        assert_eq!(params[1]["name"], "repo");
    }

    #[test]
    fn output_is_valid_json() {
        let mut table = RouteTable::new();
        table.push(make_route(Method::GET, "/health"));
        table.push(make_route_with_metadata(
            Method::GET,
            "/users/:id",
            Some("getUser"),
            &["users"],
            false,
        ));

        let info = OpenApiInfo::new("My API", "1.0.0").description("Test API");
        let json_str = table.to_openapi_json(&info);

        // Must parse as valid JSON
        let doc: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(doc["openapi"], "3.0.3");
    }
}
