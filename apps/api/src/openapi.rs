use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Pranata API",
        description = "Village website and administration API.",
        version = env!("CARGO_PKG_VERSION"),
    ),
    components(schemas(crate::error::Problem, crate::error::FieldError))
)]
pub struct ApiDoc;

/// The `OpenAPI` document, assembled from every registered path.
#[must_use]
pub fn document() -> utoipa::openapi::OpenApi {
    crate::http::api_router().split_for_parts().1
}
