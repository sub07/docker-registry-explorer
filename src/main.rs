mod auth;
mod common;
mod error;
mod home;
mod image;
mod registry;
mod router;

use std::env;

use tracing::info;

use crate::router::create_router;

#[derive(Clone)]
pub struct AppState {
    registry_api_client: registry::api::Client,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();
    common::service::env::check();

    info!("Registry Host: {}", common::service::env::registry_host());
    info!(
        "Registry Username: {}",
        common::service::env::registry_username()
    );

    let registry_api_client = registry::api::Client::new(
        common::service::env::registry_host(),
        common::service::env::registry_username(),
        common::service::env::registry_password(),
    )?;

    let app_state = AppState {
        registry_api_client,
    };

    let listen_addr = env::var("LISTEN_ADDR").expect("LISTEN_ADDR");
    let listen_port = env::var("LISTEN_PORT").expect("LISTEN_PORT");

    let binding_addr = format!("{listen_addr}:{listen_port}");

    let listener = tokio::net::TcpListener::bind(&binding_addr).await?;

    let router = create_router().with_state(app_state);

    info!("Listening on {binding_addr}");
    axum::serve(listener, router).await?;

    Ok(())
}
