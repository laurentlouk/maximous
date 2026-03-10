pub mod api;

use axum::{Router, routing::get, response::Html};
use rust_embed::Embed;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use tower_http::cors::{CorsLayer, Any};

#[derive(Embed)]
#[folder = "web/"]
struct Assets;

pub type DbState = Arc<Mutex<Connection>>;

pub fn create_router(db: DbState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(index_handler))
        .route("/app.js", get(js_handler))
        .route("/style.css", get(css_handler))
        .route("/api/overview", get(api::overview))
        .route("/api/agents", get(api::agents))
        .route("/api/tasks", get(api::tasks))
        .route("/api/messages", get(api::messages))
        .route("/api/memory", get(api::memory))
        .route("/api/sessions", get(api::sessions))
        .route("/api/changes", get(api::changes))
        .route("/api/events", get(api::events_sse))
        .layer(cors)
        .with_state(db)
}

async fn index_handler() -> Html<String> {
    match Assets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(content.data.as_ref()).to_string()),
        None => Html("<h1>maximous dashboard</h1><p>Assets not found</p>".to_string()),
    }
}

async fn js_handler() -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    let content = Assets::get("app.js")
        .map(|f| String::from_utf8_lossy(f.data.as_ref()).to_string())
        .unwrap_or_default();
    ([(axum::http::header::CONTENT_TYPE, "application/javascript")], content)
}

async fn css_handler() -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    let content = Assets::get("style.css")
        .map(|f| String::from_utf8_lossy(f.data.as_ref()).to_string())
        .unwrap_or_default();
    ([(axum::http::header::CONTENT_TYPE, "text/css")], content)
}

pub async fn serve(db: DbState, port: u16) {
    let app = create_router(db);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to bind web server");
    eprintln!("maximous dashboard: http://127.0.0.1:{}", port);
    axum::serve(listener, app).await.expect("Web server error");
}
