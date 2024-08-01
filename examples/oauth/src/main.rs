use std::env;

use anyhow::Context;
use async_session::{MemoryStore, Session, SessionStore};
use axum::extract::{FromRef, FromRequestParts, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{async_trait, RequestPartsExt, Router};
use axum_extra::typed_header::TypedHeaderRejectionReason;
use axum_extra::{headers, TypedHeader};
use http::header::SET_COOKIE;
use http::request::Parts;
use http::{header, HeaderMap, StatusCode};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

static COOKIE_NAME: &str = "SESSION";

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_oauth=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let store = MemoryStore::new();
    let oauth_client = oauth_client().unwrap();
    let app_state = AppState {
        store,
        oauth_client,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/auth/discord", get(discord_auth))
        .route("/auth/authorized", get(login_authorized))
        .route("/protected", get(protected))
        .route("/logout", get(logout))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .context("failed to bind TcpListener")
        .unwrap();

    tracing::debug!(
        "listening on {}",
        listener
            .local_addr()
            .context("failed to return local address")
            .unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    store: MemoryStore,
    oauth_client: BasicClient,
}

impl FromRef<AppState> for MemoryStore {
    fn from_ref(input: &AppState) -> Self {
        input.store.clone()
    }
}

impl FromRef<AppState> for BasicClient {
    fn from_ref(input: &AppState) -> Self {
        input.oauth_client.clone()
    }
}

fn oauth_client() -> Result<BasicClient, AppError> {
    let client_id = env::var("CLIENT_ID").context("Missing CLIENT_ID")?;
    let client_secret = env::var("CLIENT_SECRET").context("Missing CLIENT_SECRET")?;
    let redirect_url = env::var("REDIRECT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/authorized".to_string());

    let auth_url = env::var("AUTH_URL").unwrap_or_else(|_| {
        "https://discord.com/api/oauth2/authorize?response_type=code".to_string()
    });

    let token_url = env::var("TOKEN_URL")
        .unwrap_or_else(|_| "https://discord.com/api/oauth2/token".to_string());

    Ok(BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(auth_url).context("failed to create new authorization server URL")?,
        Some(TokenUrl::new(token_url).context("failed to create new token endpoint URL")?),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_url).context("failed to create new redirection URL")?,
    ))
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: String,
    avatar: Option<String>,
    username: String,
    discriminator: String,
}

async fn index(user: Option<User>) -> impl IntoResponse {
    match user {
        Some(u) => format!(
            "Hey {}! You're logged in!\nYou may now access `/protected`.\nLog out with `/logout`.",
            u.username
        ),
        None => "You're not logged in.\nVisit `/auth/discord` to do so.".to_string(),
    }
}

async fn discord_auth(State(client): State<BasicClient>) -> impl IntoResponse {
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .url();

    Redirect::to(auth_url.as_ref())
}

async fn protected(user: User) -> impl IntoResponse {
    format!("Welcome to the protected area: )\nHere's your info:\n{user:?}")
}

async fn logout(
    State(store): State<MemoryStore>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie name")?;

    let session = match store
        .load_session(cookie.to_string())
        .await
        .context("failed to load session")?
    {
        Some(s) => s,
        None => return Ok(Redirect::to("/")),
    };

    store
        .destroy_session(session)
        .await
        .context("failed to destroy session")?;

    Ok(Redirect::to("/"))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthRequest {
    code: String,
    state: String,
}

async fn login_authorized(
    Query(query): Query<AuthRequest>,
    State(store): State<MemoryStore>,
    State(oauth_client): State<BasicClient>,
) -> Result<impl IntoResponse, AppError> {
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .context("failed in sending request request to authorization server")?;

    let client = reqwest::Client::new();
    let user_data: User = client
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .context("failed in sending request to target Url")?
        .json::<User>()
        .await
        .context("failed to deserialize response as JSON")?;

    let mut session = Session::new();
    session
        .insert("user", &user_data)
        .context("failed in inserting serialized value into")?;

    let cookie = store
        .store_session(session)
        .await
        .context("failed to store session")?
        .context("unexpected error retrieving cookie value")?;

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to("/")))
}

struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/auth/discord").into_response()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = MemoryStore::from_ref(state);

        let cookies = parts
            .extract::<TypedHeader<headers::Cookie>>()
            .await
            .map_err(|e| match *e.name() {
                header::COOKIE => match e.reason() {
                    TypedHeaderRejectionReason::Missing => AuthRedirect,
                    _ => panic!("unexpected error getting Cookie header(s): {e}"),
                },
                _ => panic!("unexpected error getting cookies: {e}"),
            })?;
        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AuthRedirect)?;

        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AuthRedirect)?;

        let user = session.get::<User>("user").ok_or(AuthRedirect)?;

        Ok(user)
    }
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Application error: {:#}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}
