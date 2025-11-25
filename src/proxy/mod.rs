//! Proxy module
//!
//! Forwards requests to a target server and records response status codes.

use crate::config::SharedConfig;
use crate::metrics::{RequestMetric, SharedMetrics};
use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use reqwest::Client;
use std::sync::Arc;
use std::time::Instant;

/// Proxy state containing shared configuration and HTTP client
#[derive(Clone)]
pub struct ProxyState {
    pub config: SharedConfig,
    pub metrics: SharedMetrics,
    pub client: Client,
}

impl ProxyState {
    /// Create a new proxy state
    pub fn new(config: SharedConfig, metrics: SharedMetrics) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            metrics,
            client,
        }
    }
}

/// Proxy handler that forwards requests to the target server
pub async fn proxy_handler(
    State(state): State<Arc<ProxyState>>,
    req: Request<Body>,
) -> impl IntoResponse {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let config = state.config.get();

    // Check if proxy is enabled
    if !config.proxy.enabled {
        let metric = RequestMetric::new(method, path)
            .with_status(200)
            .with_latency(start.elapsed().as_secs_f64() * 1000.0)
            .with_proxied(false);
        state.metrics.record(metric);

        return (StatusCode::OK, "Proxy mode disabled").into_response();
    }

    // Get target URL
    let target = match &config.proxy.target {
        Some(t) => t.clone(),
        None => {
            let metric = RequestMetric::new(method, path)
                .with_status(502)
                .with_latency(start.elapsed().as_secs_f64() * 1000.0)
                .with_proxied(false);
            state.metrics.record(metric);

            return (StatusCode::BAD_GATEWAY, "No proxy target configured").into_response();
        }
    };

    // Build the proxied URL
    let uri = req.uri();
    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

    let proxied_url = format!("{}{}", target.trim_end_matches('/'), path_and_query);

    // Forward the request
    let result = forward_request(&state.client, req, &proxied_url).await;

    let latency = start.elapsed().as_secs_f64() * 1000.0;

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let metric = RequestMetric::new(method, path)
                .with_status(status)
                .with_latency(latency)
                .with_proxied(true);
            state.metrics.record(metric);

            tracing::info!(
                target = %proxied_url,
                status = %status,
                latency_ms = %latency,
                "Proxied request"
            );

            response.into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, target = %proxied_url, "Proxy error");

            let metric = RequestMetric::new(method, path)
                .with_status(502)
                .with_latency(latency)
                .with_proxied(true);
            state.metrics.record(metric);

            (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e)).into_response()
        }
    }
}

/// Forward a request to the target URL
async fn forward_request(
    client: &Client,
    req: Request<Body>,
    target_url: &str,
) -> Result<Response<Body>> {
    let method = req.method().clone();
    let headers = req.headers().clone();

    // Read the request body
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX).await?;

    // Build the forwarded request
    let mut builder = client.request(
        method.to_string().parse().unwrap_or(reqwest::Method::GET),
        target_url,
    );

    // Copy headers (excluding host)
    for (key, value) in headers.iter() {
        if key != "host" {
            if let Ok(v) = value.to_str() {
                builder = builder.header(key.as_str(), v);
            }
        }
    }

    // Set body if present
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    // Send the request
    let response = builder.send().await?;

    // Convert response
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = response.bytes().await?;

    let mut response_builder = Response::builder().status(status.as_u16());

    for (key, value) in headers.iter() {
        response_builder = response_builder.header(key, value);
    }

    let response = response_builder
        .body(Body::from(body_bytes.to_vec()))
        .unwrap();

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::metrics::create_shared_metrics;

    #[test]
    fn test_proxy_state_creation() {
        let config = SharedConfig::new(AppConfig::default());
        let metrics = create_shared_metrics(1000);
        let state = ProxyState::new(config, metrics);

        // Just verify it can be created
        assert!(!state.config.get().proxy.enabled);
    }
}
