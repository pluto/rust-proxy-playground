//! Integration tests
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(non_snake_case)]
#![allow(clippy::clone_on_copy)]
use std::time::Duration;

use anyhow::Result;
use once_cell::sync::Lazy;
use rstest::*;
use tokio::time::sleep;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::fmt::format::FmtSpan;

static TEST_PORT: Lazy<String> = Lazy::new(|| "3333".to_string());
static TEST_SERVER: Lazy<String> = Lazy::new(|| format!("http://localhost:{}", *TEST_PORT));

/// Helper struct to manage server lifecycle
#[derive(Debug)]
struct TestServer {
    handle: tokio::process::Child,
}

impl Drop for TestServer {
    fn drop(&mut self) { let _ = self.handle.start_kill(); }
}

impl TestServer {
    async fn start() -> Result<Self> {
        info!("Starting test server on port {}", *TEST_PORT);
        let handle = tokio::process::Command::new("cargo")
            .args(["run", "-p", "proxy", "--"])
            .env("PROXY_PORT", &*TEST_PORT)  // Changed from PORT to PROXY_PORT
            .env("RUST_LOG", "debug")
            .kill_on_drop(true)
            .spawn()?;

        // Wait longer for server startup
        info!("Waiting for server to start up");
        sleep(Duration::from_secs(2)).await; // Increased from 5ms to 2s

        // Verify server is running by attempting to connect
        let client = reqwest::Client::new();
        let max_retries = 5;
        for i in 0..max_retries {
            match client.get(&*TEST_SERVER).send().await {
                Ok(_) => {
                    info!("Server is ready!");
                    break;
                },
                Err(e) => {
                    if i == max_retries - 1 {
                        error!("Server failed to start after {} retries: {}", max_retries, e);
                        return Err(anyhow::anyhow!("Server failed to start"));
                    }
                    warn!("Server not ready, retrying in 1s...");
                    sleep(Duration::from_secs(1)).await;
                },
            }
        }

        Ok(Self { handle })
    }

    #[instrument]
    async fn stop(mut self) -> Result<()> {
        info!("Stopping test server");
        self.handle.kill().await?;
        Ok(())
    }
}

/// Test fixture for managing server and client setup
#[derive(Debug)]
struct TestContext {
    server: TestServer,
    client: reqwest::Client,
}

/// Initialize tracing for tests
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_span_events(FmtSpan::CLOSE)
        .with_test_writer()
        .try_init();
}

/// Fixture setup function
#[fixture]
async fn test_context() -> TestContext {
    init_tracing();
    info!("Setting up test context");
    let server = TestServer::start().await.expect("Failed to start test server");
    let client = reqwest::Client::new();
    TestContext { server, client }
}

#[rstest]
#[tokio::test]
async fn test_proxy_forwarding(#[future] test_context: TestContext) -> Result<()> {
    let ctx = test_context.await;
    let test_url = "https://gist.githubusercontent.com/mattes/23e64faadb5fd4b5112f379903d2572e/raw/ddbf0a56001367467f71bda64347aa881d83533c/example.json";

    info!("Sending request through proxy to {}", test_url);
    let response = ctx.client.get(&*TEST_SERVER).header("X-Target-URL", test_url).send().await?;

    debug!(status = ?response.status(), "Received response");
    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await?;
    debug!(body = ?body, "Parsed response body");
    assert_eq!(body["hello"], "world");

    ctx.server.stop().await?;
    Ok(())
}
