use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

/// Registers the cookie security scheme the authenticated paths refer to.
struct SessionCookie;

impl Modify for SessionCookie {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(
                crate::auth::session::COOKIE_NAME,
            ))),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Pranata API",
        description = "Village website and administration API.",
        version = env!("CARGO_PKG_VERSION"),
    ),
    components(schemas(crate::error::Problem, crate::error::FieldError)),
    modifiers(&SessionCookie)
)]
pub struct ApiDoc;

/// The `OpenAPI` document, assembled from every registered path.
#[must_use]
pub fn document() -> utoipa::openapi::OpenApi {
    crate::http::api_router().split_for_parts().1
}
