//! JSON request bodies that fail with a problem document.

use axum::{
    extract::{FromRequest, Request, rejection::JsonRejection},
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;

use crate::ApiError;

/// Like [`axum::Json`] as an extractor, but every rejection is an
/// `application/problem+json` document: 415 `json_required` when the request
/// is not marked as JSON, 400 `invalid_json` for anything else.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(JsonRejection::MissingJsonContentType(_)) => {
                Err(ApiError::Unsupported("json_required").into_response())
            }
            Err(_) => Err(ApiError::BadRequest("invalid_json").into_response()),
        }
    }
}
