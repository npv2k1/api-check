//! HTTP Server module
//!
//! Provides the main HTTP server with request counting and timing middleware.

use crate::api::{create_api_router, ApiState};
use crate::config::SharedConfig;
use crate::metrics::{RequestMetric, SharedMetrics};
use crate::proxy::{proxy_handler, ProxyState};
use crate::testing::SharedTester;
use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::any,
    Router,
};
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Server state
#[derive(Clone)]
pub struct ServerState {
    pub config: SharedConfig,
    pub metrics: SharedMetrics,
    pub tester: SharedTester,
}

impl ServerState {
    pub fn new(config: SharedConfig, metrics: SharedMetrics, tester: SharedTester) -> Self {
        Self {
            config,
            metrics,
            tester,
        }
    }
}

/// Request timing and counting middleware
pub async fn metrics_middleware(
    metrics: SharedMetrics,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Don't record metrics for API endpoints
    let skip_metrics = path.starts_with("/api/");

    let response = next.run(req).await;

    if !skip_metrics {
        let latency = start.elapsed().as_secs_f64() * 1000.0;
        let status = response.status().as_u16();

        let metric = RequestMetric::new(method.clone(), path.clone())
            .with_status(status)
            .with_latency(latency);
        metrics.record(metric);

        tracing::debug!(
            method = %method,
            path = %path,
            status = %status,
            latency_ms = %latency,
            "Request processed"
        );
    }

    response
}

/// Create the main server router
pub fn create_server_router(state: Arc<ServerState>) -> Router {
    // Create API state
    let api_state = Arc::new(ApiState::new(
        state.config.clone(),
        state.metrics.clone(),
        state.tester.clone(),
    ));

    // Create proxy state
    let proxy_state = Arc::new(ProxyState::new(state.config.clone(), state.metrics.clone()));

    // Clone metrics for middleware
    let metrics_for_middleware = state.metrics.clone();

    // Create the router
    Router::new()
        // Management API routes
        .merge(create_api_router(api_state))
        // Dev server routes - catch all for proxy/echo
        .route("/", any(dev_handler))
        .route(
            "/*path",
            any(move |req| proxy_or_echo(proxy_state.clone(), req)),
        )
        // Add middleware
        .layer(middleware::from_fn(move |req, next| {
            metrics_middleware(metrics_for_middleware.clone(), req, next)
        }))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

/// Dev handler for root path
async fn dev_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        "API Check Dev Server - Use /api/* for management endpoints",
    )
}

/// Proxy or echo handler for all other paths
async fn proxy_or_echo(proxy_state: Arc<ProxyState>, req: Request<Body>) -> impl IntoResponse {
    let config = proxy_state.config.get();

    if config.proxy.enabled && config.proxy.target.is_some() {
        // Forward to proxy
        proxy_handler(axum::extract::State(proxy_state), req)
            .await
            .into_response()
    } else {
        // Echo request details
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let headers: Vec<(String, String)> = req
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let response = serde_json::json!({
            "method": method,
            "path": path,
            "headers": headers,
            "message": "Echo response from dev server"
        });

        (StatusCode::OK, axum::Json(response)).into_response()
    }
}

/// Start the HTTP server
pub async fn start_server(
    config: SharedConfig,
    metrics: SharedMetrics,
    tester: SharedTester,
) -> anyhow::Result<()> {
    let server_config = config.get().server;
    let addr = format!("{}:{}", server_config.host, server_config.port);

    let state = Arc::new(ServerState::new(config, metrics, tester));
    let app = create_server_router(state);

    tracing::info!(addr = %addr, "Starting HTTP server");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::metrics::create_shared_metrics;
    use crate::testing::create_shared_tester;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn create_test_app() -> Router {
        let config = SharedConfig::new(AppConfig::default());
        let metrics = create_shared_metrics(1000);
        let tester = create_shared_tester(config.clone(), metrics.clone());
        let state = Arc::new(ServerState::new(config, metrics, tester));
        create_server_router(state)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_test_app();

        let request = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_dev_handler() {
        let app = create_test_app();

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_echo_handler() {
        let app = create_test_app();

        let request = Request::builder()
            .uri("/test/path")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
