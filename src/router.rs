use std::env;

use axum::{
    Router,
    response::Redirect,
    routing::{get, post},
};
use tower_http::services::ServeDir;

use crate::{AppState, auth, common, home, image};

pub fn create_router() -> Router<AppState> {
    let static_dir = env::var("STATIC_DIR").expect("STATIC_DIR");

    Router::new()
        .route("/", get(home::handler::index))
        .route("/{image}", get(image::handler::index))
        .route(
            "/{image}/delete",
            post(home::handler::delete_all_image_tags),
        )
        .route("/{image}/delete/{digest}", post(image::handler::delete_tag))
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/static/favicon.ico") }),
        )
        .route("/auth/login", get(auth::handler::login_index))
        .route("/auth/authenticate", post(auth::handler::authenticate))
        .route("/auth/logout", post(auth::handler::logout))
        .route("/health", get(common::handler::health))
        .nest_service("/static", ServeDir::new(static_dir))
}
