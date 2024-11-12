//! Client to connect to a proxy re-encryption server
// #![allow(unused_imports)]
// #![allow(unused_variables)]
// #![allow(dead_code)]
// #![allow(unreachable_code)]
// #![allow(non_snake_case)]
// #![allow(unused_mut)]

use std::env;

use anyhow::{Context, Result};
use dotenv::dotenv;
use serde::Deserialize;
use tracing::{debug, error, info, warn};

#[derive(Clone, Debug, Deserialize)]
struct Response {
    _hello: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_init();
    info!("Starting proxy client");

    let proxy_addr = env::var("PROXY_ADDRESS").unwrap_or_else(|_| {
        let default = "http://localhost:3000".to_string();
        warn!("PROXY_ADDRESS not set, using default: {}", default);
        default
    });
    let target_url = env::var("TARGET_URL")
        .unwrap_or_else(|_| {
            let default = "https://gist.githubusercontent.com/mattes/23e64faadb5fd4b5112f379903d2572e/raw/ddbf0a56001367467f71bda64347aa881d83533c/example.json".to_string();
            warn!("TARGET_URL not set, using default: {}", default);
            default
        });

    info!(
        proxy_addr = %proxy_addr,
        target_url = %target_url,
        "Sending request through proxy"
    );
    let response = reqwest::Client::new()
        .get(&proxy_addr)
        .header("X-Target-URL", target_url)
        .send()
        .await
        .context("Failed to send request")?;
    debug!(
        status = %response.status(),
        headers = ?response.headers(),
        "Received response from proxy"
    );

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        error!(
            status = %status,
            error_body = %text,
            "Request failed"
        );
        anyhow::bail!("Request failed with status {}: {}", status, text);
    }

    let data: Response = response.json().await.context("Failed to parse response as JSON")?;
    info!(
        response = ?data,
        "Successfully received and parsed response"
    );

    Ok(())
}

fn tracing_init() {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,proxy_client=debug".into()))
        .init();
}
