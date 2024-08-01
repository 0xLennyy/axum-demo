use std::net::SocketAddr;
use std::str::FromStr;

use axum::extract::State;
use axum::handler::Handler;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use diesel::query_dsl::select_dsl::SelectDsl;
use diesel::{table, Insertable, Queryable, RunQueryDsl, Selectable, SelectableHelper};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde::{Deserialize, Serialize};
use tracing::error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

table! {
    users (id) {
        id -> Integer,
        name -> Text,
        hair_color -> Nullable<Text>
    }
}

#[derive(Serialize, Selectable, Queryable)]
struct User {
    id: i32,
    name: String,
    hair_color: Option<String>,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = users)]
struct NewUser {
    name: String,
    hair_color: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_tokio_postgres=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = std::env::var("DATABASE_URL").unwrap();

    let manager = deadpool_diesel::postgres::Manager::new(db_url, deadpool_diesel::Runtime::Tokio1);
    let pool = deadpool_diesel::postgres::Pool::builder(manager)
        .build()
        .unwrap();

    {
        let conn = pool.get().await.unwrap();
        conn.interact(|conn| conn.run_pending_migrations(MIGRATIONS).map(|_| ()))
            .await
            .unwrap()
            .unwrap();
    }

    let app = Router::new()
        .route("/user/list", get(list_users))
        .route("/user/create", post(create_user))
        .with_state(pool);

    let addr = SocketAddr::from_str("127.0.0.1:3000").unwrap();
    tracing::debug!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_user(
    State(pool): State<deadpool_diesel::postgres::Pool>,
    Json(new_user): Json<NewUser>,
) -> Result<Json<User>, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let res = conn
        .interact(|conn| {
            diesel::insert_into(users::table)
                .values(new_user)
                .returning(User::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;

    Ok(Json(res))
}

async fn list_users(
    State(pool): State<deadpool_diesel::postgres::Pool>,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let res = conn
        .interact(|conn| users::table.select(User::as_select()).load(conn))
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    Ok(Json(res))
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
