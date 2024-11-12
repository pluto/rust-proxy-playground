//! Proxy re-encryption server that intercepts and forwards requests
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(non_snake_case)]
#![allow(clippy::clone_on_copy)]
#![allow(unused_mut)]
// #[cfg(test)] mod tests;

use std::env;

use anyhow::Result;
use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use once_cell::sync::Lazy;
use serde_json::Value;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, instrument, warn};

static CLIENT: Lazy<reqwest::Client> =
    Lazy::new(|| reqwest::Client::builder().build().expect("Failed to create HTTP client"));

static CONFIG: Lazy<ProxyConfig> = Lazy::new(|| ProxyConfig {
    port: env::var("PROXY_PORT")
        .unwrap_or_else(|_| {
            let default = "3000".to_string();
            warn!("PORT not set, using default: {}", default);
            default
        })
        .parse()
        .expect("Failed to parse PORT"),
});

#[derive(Debug)]
struct ProxyConfig {
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    tracing_setup();

    info!("Starting proxy re-encryption server");
    let app = Router::new().route("/", get(handle_request)).layer(TraceLayer::new_for_http());
    let addr = format!("0.0.0.0:{}", CONFIG.port);
    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

fn tracing_setup() {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,proxy=debug".into()))
        .init();
}

/// Extract target URL from headers
#[instrument(skip(headers))]
async fn handle_request(headers: HeaderMap) -> impl IntoResponse {
    let target_url = match headers.get("x-target-url") {
        Some(target) => target.to_str().unwrap_or_default(),
        None => {
            error!("No target URL provided");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Missing X-Target-URL header"
                })),
            )
                .into_response();
        },
    };

    info!(target_url, "Received proxy request");

    // Simulate re-encryption process
    debug!("Simulating re-encryption of request");
    // TODO: Implement actual re-encryption logic

    // Forward the request to the target URL
    match forward_request(target_url).await {
        Ok(response) => response.into_response(),
        Err(e) => {
            error!(error = ?e, "Failed to forward request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to forward request: {}", e)
                })),
            )
                .into_response()
        },
    }
}

async fn forward_request(target_url: &str) -> Result<impl IntoResponse> {
    debug!(target_url, "Forwarding request");

    // Make the request to the target URL
    let response = CLIENT.get(target_url).send().await?;

    let status = StatusCode::from_u16(response.status().as_u16())?;
    let body: Value = response.json().await?;

    debug!(status = ?status, "Received response from target");

    Ok((status, Json(body)))
}
