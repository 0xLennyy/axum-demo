use std::convert::Infallible;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::handler::Handler;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use reqwest::{Client, StatusCode};
use tokio_stream::StreamExt;
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_reqwest_response=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = Client::new();

    let app = Router::new()
        .route("/", get(proxy_via_reqwest))
        .route("/stream", get(stream_some_data))
        .layer(TraceLayer::new_for_http().on_body_chunk(
            |chunk: &Bytes, _latency: Duration, _span: &Span| {
                tracing::debug!("streaming {} bytes", chunk.len());
            },
        ))
        .with_state(client);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn proxy_via_reqwest(State(client): State<Client>) -> Response {
    let reqwest_response = match client.get("http://127.0.0.1:3000/stream").send().await {
        Ok(res) => res,
        Err(err) => {
            tracing::error!(%err,"request failed");
            return (StatusCode::BAD_REQUEST, Body::empty()).into_response();
        }
    };

    let response_builder = Response::builder().status(reqwest_response.status().as_u16());

    let mut headers = HeaderMap::with_capacity(reqwest_response.headers().len());
    headers.extend(reqwest_response.headers().into_iter().map(|(name, value)| {
        let name = HeaderName::from_bytes(name.as_ref()).unwrap();
        let value = HeaderValue::from_bytes(value.as_ref()).unwrap();
        (name, value)
    }));

    tracing::debug!("headers: {:?}", headers);

    response_builder
        .body(Body::from_stream(reqwest_response.bytes_stream()))
        .unwrap()
}

async fn stream_some_data() -> Body {
    let stream = tokio_stream::iter(0..5)
        .throttle(Duration::from_secs(1))
        .map(|n| n.to_string())
        .map(Ok::<_, Infallible>);
    Body::from_stream(stream)
}
