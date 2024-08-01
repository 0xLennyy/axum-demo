use axum::response::{Html, Response};
use axum::routing::get;
use axum::Router;
use futures_executor::block_on;
use http::Request;
use tower_service::Service;

fn main() {
    let request: Request<String> = Request::builder()
        .uri("https://serverless.example/api/")
        .body("Some Body Data".into())
        .unwrap();

    let response: Response = block_on(app(request));
    assert_eq!(200, response.status());
}

async fn app(request: Request<String>) -> Response {
    let mut router = Router::new().route("/api/", get(index));
    let response = router.call(request).await.unwrap();
    response
}

async fn index() -> Html<&'static str> {
    Html("<h1>Hello, World</h1>")
}
