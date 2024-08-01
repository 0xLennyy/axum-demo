use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, MatchedPath, Request};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{async_trait, RequestPartsExt};
use serde_json::{json, Value};

pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    axum::Json(dbg!(value))
}

pub struct Json<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, axum::Json<Value>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();

        let path = parts
            .extract::<MatchedPath>()
            .await
            .map(|path| path.as_str().to_owned())
            .ok();

        let req = Request::from_parts(parts, body);

        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                let payload = json!({
                    "message": rejection.body_text(),
                    "origin": "custom_extractor",
                    "path": path
                });

                Err((rejection.status(), axum::Json(payload)))
            }
        }
    }
}
