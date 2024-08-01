use std::time::Duration;

use axum::body::Bytes;
use axum::extract::MatchedPath;
use axum::http::{HeaderMap, Request};
use axum::response::{Html, Response};
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                "example_tracing_aka_logging=debug,tower_http=debug,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str);

                    info_span!("http_request", method = ?request.method(), matched_path, some_other_field = tracing::field::Empty)
                })
                .on_request(|_request: &Request<_>, _span: &Span| {})
                .on_response(|_response: &Response, _latency: Duration, _span: &Span| {})
                .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {})
                .on_eos(|_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span| {})
                .on_failure(|_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {})
    );

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World</h1>")
}
