use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use minijinja::{context, Environment};

struct AppState {
    env: Environment<'static>,
}

#[tokio::main]
async fn main() {
    let mut env = Environment::new();
    env.add_template("layout", include_str!("../templates/layout.jinja"))
        .unwrap();
    env.add_template("home", include_str!("../templates/home.jinja"))
        .unwrap();
    env.add_template("content", include_str!("../templates/content.jinja"))
        .unwrap();
    env.add_template("about", include_str!("../templates/about.jinja"))
        .unwrap();

    let app_state = Arc::new(AppState { env });

    let app = Router::new()
        .route("/", get(handler_home))
        .route("/content", get(handler_content))
        .route("/about", get(handler_about))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler_home(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    let template = state.env.get_template("home").unwrap();

    let rendered = template
        .render(context! {
            title => "Home",
            welcome_text => "Hello World"
        })
        .unwrap();

    Ok(Html(rendered))
}

async fn handler_content(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    let template = state.env.get_template("content").unwrap();

    let some_example_entries = vec!["Data 1", "Data 2", "Date 3"];

    let rendered = template
        .render(context! {
            title => "Content",
            entries => some_example_entries
        })
        .unwrap();

    Ok(Html(rendered))
}

async fn handler_about(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    let template = state.env.get_template("about").unwrap();

    let rendered = template.render(context! {
        title => "About",
        about_text => "Simple demonstration layout for an axum project with minijinja as templating engine."
    }).unwrap();

    Ok(Html(rendered))
}
