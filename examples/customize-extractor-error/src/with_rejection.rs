use axum::extract::rejection::JsonRejection;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::WithRejection;
use serde_json::{json, Value};
use thiserror::Error;

pub async fn handler(
    WithRejection(Json(value), _): WithRejection<Json<Value>, ApiError>,
) -> impl IntoResponse {
    Json(dbg!(value))
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::JsonExtractorRejection(json_rejection) => {
                (json_rejection.status(), json_rejection.body_text())
            }
        };

        let payload = json!({
            "message": message,
            "origin": "with_rejection"
        });

        (status, Json(payload)).into_response()
    }
}
