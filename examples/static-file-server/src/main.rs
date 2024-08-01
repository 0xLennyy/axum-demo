use std::net::SocketAddr;

use axum::extract::Request;
use axum::handler::HandlerWithoutStateExt;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_static_file_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tokio::join!(
        serve(using_serve_dir(), 3001),
        serve(using_serve_dir_with_assets_fallback(), 3002),
        serve(using_serve_dir_only_from_root_via_fallback(), 3003),
        serve(using_serve_dir_with_handler_as_service(), 3004),
        serve(two_serve_dirs(), 3005),
        serve(calling_serve_dir_from_a_handler(), 3006),
        serve(using_serve_file_from_a_route(), 3007)
    );
}

fn using_serve_dir() -> Router {
    Router::new().nest_service("/assets", ServeDir::new("assets"))
}

fn using_serve_dir_with_assets_fallback() -> Router {
    let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .nest_service("/assets", serve_dir.clone())
        .fallback_service(serve_dir)
}

fn using_serve_dir_only_from_root_via_fallback() -> Router {
    let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback_service(serve_dir)
}

fn using_serve_dir_with_handler_as_service() -> Router {
    async fn handle_404() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "Not found")
    }

    let service = handle_404.into_service();

    let serve_dir = ServeDir::new("assets").not_found_service(service);

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback_service(serve_dir)
}

fn two_serve_dirs() -> Router {
    let serve_dir_from_assets = ServeDir::new("assets");
    let serve_dir_from_dist = ServeDir::new("dist");

    Router::new()
        .nest_service("/assets", serve_dir_from_assets)
        .nest_service("/dist", serve_dir_from_dist)
}

fn calling_serve_dir_from_a_handler() -> Router {
    Router::new().nest_service(
        "/foo",
        get(|request: Request| async {
            let service = ServeDir::new("assets");
            let result = service.oneshot(request).await;
            result
        }),
    )
}

fn using_serve_file_from_a_route() -> Router {
    Router::new().route_service("/foo", ServeFile::new("assets/index.html"))
}

async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}
