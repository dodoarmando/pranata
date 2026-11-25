use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

/// One invalid field in a validation failure.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FieldError {
    pub field: String,
    pub code: String,
}

/// `RFC 9457` problem document returned for every error.
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    /// Stable machine-readable code (snake case).
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FieldError>>,
}

/// Every error an endpoint can return. The `&'static str` payloads are the
/// problem `code`, in `snake_case`.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(&'static str),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("unauthorized: {0}")]
    Unauthorized(&'static str),
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error("gone: {0}")]
    Gone(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("payload too large: {0}")]
    TooLarge(&'static str),
    #[error("unprocessable: {0}")]
    Unprocessable(&'static str),
    #[error("unavailable: {0}")]
    Unavailable(&'static str),
    #[error("too many requests: {code}")]
    TooManyRequests {
        code: &'static str,
        /// Seconds until the caller may try again (the `Retry-After` header).
        retry_after: u64,
    },
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, &'static str) {
        match self {
            Self::BadRequest(code) => (StatusCode::BAD_REQUEST, "Bad Request", code),
            Self::NotFound(code) => (StatusCode::NOT_FOUND, "Not Found", code),
            Self::Unauthorized(code) => (StatusCode::UNAUTHORIZED, "Unauthorized", code),
            Self::Forbidden(code) => (StatusCode::FORBIDDEN, "Forbidden", code),
            Self::Conflict(code) => (StatusCode::CONFLICT, "Conflict", code),
            Self::Gone(code) => (StatusCode::GONE, "Gone", code),
            Self::Unsupported(code) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Unsupported Media Type",
                code,
            ),
            Self::TooLarge(code) => (StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large", code),
            Self::Unprocessable(code) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Unprocessable Entity",
                code,
            ),
            Self::Unavailable(code) => {
                (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable", code)
            }
            Self::TooManyRequests { code, .. } => {
                (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests", code)
            }
            Self::Validation(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Unprocessable Entity",
                "validation_failed",
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "internal_error",
            ),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, code) = self.parts();
        if let Self::Internal(error) = &self {
            tracing::error!(error = ?error, "internal error");
        }
        let retry_after = match &self {
            Self::TooManyRequests { retry_after, .. } => Some(*retry_after),
            _ => None,
        };
        let errors = match self {
            Self::Validation(errors) => Some(errors),
            _ => None,
        };
        let problem = Problem {
            kind: "about:blank".to_owned(),
            title: title.to_owned(),
            status: status.as_u16(),
            code: code.to_owned(),
            errors,
        };
        let mut response = (status, Json(problem)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/problem+json"),
        );
        if let Some(seconds) = retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, header::HeaderValue::from(seconds));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;
    use serde_json::{Value, json};

    use super::*;

    async fn render(error: ApiError) -> (StatusCode, Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn validation_lists_field_errors() {
        let (status, body) = render(ApiError::Validation(vec![FieldError {
            field: "name".into(),
            code: "required".into(),
        }]))
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(
            body["errors"],
            json!([{"field": "name", "code": "required"}])
        );
    }

    #[tokio::test]
    async fn internal_errors_do_not_leak_details() {
        let (status, body) = render(ApiError::Internal(anyhow::anyhow!("db password: x"))).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "internal_error");
        assert!(!body.to_string().contains("password"));
        assert!(body.get("errors").is_none());
    }

    #[test]
    fn too_many_requests_sets_retry_after() {
        let response = ApiError::TooManyRequests {
            code: "too_many_attempts",
            retry_after: 900,
        }
        .into_response();
        assert_eq!(response.headers()[header::RETRY_AFTER], "900");
    }

    #[tokio::test]
    async fn statuses_follow_variants() {
        for (error, expected) in [
            (ApiError::BadRequest("invalid_json"), 400),
            (ApiError::Unauthorized("unauthenticated"), 401),
            (ApiError::Forbidden("csrf_mismatch"), 403),
            (ApiError::Conflict("access_link_used"), 409),
            (ApiError::Gone("gone"), 410),
            (ApiError::Unsupported("unsupported_media_type"), 415),
            (ApiError::TooLarge("file_too_large"), 413),
            (ApiError::Unprocessable("image_too_large"), 422),
            (
                ApiError::TooManyRequests {
                    code: "too_many_attempts",
                    retry_after: 60,
                },
                429,
            ),
        ] {
            assert_eq!(render(error).await.0.as_u16(), expected);
        }
    }
}
