use axum::body::Bytes;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::handler::Handler;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{BoxError, Router};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_key_value_store=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let shared_state = SharedState::default();

    let app = Router::new()
        .route("/:key", get(kv_get.layer(CompressionLayer::new())))
        .route(
            "/:key",
            post(kv_set.layer((
                DefaultBodyLimit::disable(),
                RequestBodyLimitLayer::new(1024 * 5_000),
            ))),
        )
        .route("/keys", get(list_keys))
        .nest("/admin", admin_routes())
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(Arc::clone(&shared_state));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

type SharedState = Arc<RwLock<AppState>>;

#[derive(Default)]
struct AppState {
    db: HashMap<String, Bytes>,
}

async fn kv_get(
    Path(key): Path<String>,
    State(state): State<SharedState>,
) -> Result<Bytes, StatusCode> {
    let db = &state.read().await.db;

    if let Some(value) = db.get(&key) {
        Ok(value.clone())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn kv_set(Path(key): Path<String>, State(state): State<SharedState>, bytes: Bytes) {
    state.write().await.db.insert(key, bytes);
}

async fn list_keys(State(state): State<SharedState>) -> String {
    let db = &state.read().await.db;

    db.keys()
        .map(|key| key.to_string())
        .collect::<Vec<String>>()
        .join("\n")
}

fn admin_routes() -> Router<SharedState> {
    async fn delete_all_keys(State(state): State<SharedState>) {
        state.write().await.db.clear();
    }

    async fn remove_key(Path(key): Path<String>, State(state): State<SharedState>) {
        state.write().await.db.remove(&key);
    }

    Router::new()
        .route("/keys", delete(delete_all_keys))
        .route("/key/:key", delete(remove_key))
        .layer(ValidateRequestHeaderLayer::bearer("secret-token"))
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request time out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {error}")),
    )
}
