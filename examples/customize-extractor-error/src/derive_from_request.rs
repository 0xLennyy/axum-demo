use axum::extract::FromRequest;
use axum::response::Response;
use axum::{extract::rejection::JsonRejection, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use serde_json::{json, Value};

pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value))
}

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        let Self(value) = self;
        axum::Json(value).into_response()
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<JsonRejection> for ApiError {
    fn from(value: JsonRejection) -> Self {
        Self {
            status: value.status(),
            message: value.body_text(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let payload = json!({
            "message": self.message,
            "origin": "derive_from_request"
        });

        (self.status, axum::Json(payload)).into_response()
    }
}
