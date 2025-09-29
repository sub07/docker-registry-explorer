use std::env;

use axum::{Router, routing::get};
use tower_http::services::ServeDir;

use crate::{AppState, home, image};

pub fn create_router() -> Router<AppState> {
    let static_dir = env::var("STATIC_DIR").expect("STATIC_DIR");

    Router::new()
        .route("/", get(home::handler::index))
        .route("/{image}", get(image::handler::index))
        .nest_service("/static", ServeDir::new(static_dir))
}
