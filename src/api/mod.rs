//! Management API module
//!
//! Provides HTTP endpoints for configuration management and metrics export.

use crate::config::{AppConfig, ProxyConfig, SharedConfig, TestConfig};
use crate::metrics::{MetricsSummary, RequestMetric, SharedMetrics};
use crate::testing::SharedTester;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub config: SharedConfig,
    pub metrics: SharedMetrics,
    pub tester: SharedTester,
}

impl ApiState {
    pub fn new(config: SharedConfig, metrics: SharedMetrics, tester: SharedTester) -> Self {
        Self {
            config,
            metrics,
            tester,
        }
    }
}

/// Create the management API router
///
/// Note: The PUT /api/config endpoint replaces the entire configuration.
/// For production use, consider implementing PATCH endpoints for partial updates.
pub fn create_api_router(state: Arc<ApiState>) -> Router {
    Router::new()
        // Configuration endpoints
        .route("/api/config", get(get_config).put(update_config))
        .route(
            "/api/config/proxy",
            get(get_proxy_config).put(update_proxy_config),
        )
        .route(
            "/api/config/test",
            get(get_test_config).put(update_test_config),
        )
        // Metrics endpoints
        .route("/api/metrics", get(get_metrics))
        .route("/api/metrics/summary", get(get_metrics_summary))
        .route("/api/metrics/recent", get(get_recent_metrics))
        .route("/api/metrics/clear", post(clear_metrics))
        // Test endpoints
        .route("/api/test/run", post(run_test))
        .route("/api/test/status", get(get_test_status))
        .route("/api/test/stop", post(stop_test))
        // Health check
        .route("/api/health", get(health_check))
        .with_state(state)
}

/// Get current configuration
async fn get_config(State(state): State<Arc<ApiState>>) -> Json<AppConfig> {
    Json(state.config.get())
}

/// Update configuration
async fn update_config(
    State(state): State<Arc<ApiState>>,
    Json(config): Json<AppConfig>,
) -> impl IntoResponse {
    state.config.update(config);
    (StatusCode::OK, "Configuration updated")
}

/// Get proxy configuration
async fn get_proxy_config(State(state): State<Arc<ApiState>>) -> Json<ProxyConfig> {
    Json(state.config.get().proxy)
}

/// Update proxy configuration
#[derive(Debug, Deserialize)]
pub struct UpdateProxyRequest {
    pub enabled: Option<bool>,
    pub target: Option<String>,
}

async fn update_proxy_config(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<UpdateProxyRequest>,
) -> impl IntoResponse {
    let mut current = state.config.get().proxy;

    if let Some(enabled) = req.enabled {
        current.enabled = enabled;
    }
    if req.target.is_some() {
        current.target = req.target;
    }

    state.config.update_proxy(current);
    (StatusCode::OK, "Proxy configuration updated")
}

/// Get test configuration
async fn get_test_config(State(state): State<Arc<ApiState>>) -> Json<TestConfig> {
    Json(state.config.get().test)
}

/// Update test configuration
#[derive(Debug, Deserialize)]
pub struct UpdateTestRequest {
    pub num_calls: Option<u32>,
    pub frequency_ms: Option<u64>,
    pub method: Option<String>,
    pub target_url: Option<String>,
    pub body: Option<String>,
    pub headers: Option<Vec<(String, String)>>,
}

async fn update_test_config(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<UpdateTestRequest>,
) -> impl IntoResponse {
    let mut current = state.config.get().test;

    if let Some(num_calls) = req.num_calls {
        current.num_calls = num_calls;
    }
    if let Some(frequency_ms) = req.frequency_ms {
        current.frequency_ms = frequency_ms;
    }
    if let Some(method) = req.method {
        current.method = method;
    }
    if req.target_url.is_some() {
        current.target_url = req.target_url;
    }
    if req.body.is_some() {
        current.body = req.body;
    }
    if let Some(headers) = req.headers {
        current.headers = headers;
    }

    state.config.update_test(current);
    (StatusCode::OK, "Test configuration updated")
}

/// Get all metrics
async fn get_metrics(State(state): State<Arc<ApiState>>) -> Json<Vec<RequestMetric>> {
    Json(state.metrics.get_all())
}

/// Get metrics summary
async fn get_metrics_summary(State(state): State<Arc<ApiState>>) -> Json<MetricsSummary> {
    Json(state.metrics.get_summary())
}

/// Query parameters for recent metrics
#[derive(Debug, Deserialize, Default)]
pub struct RecentMetricsQuery {
    #[serde(default = "default_seconds")]
    pub seconds: i64,
}

fn default_seconds() -> i64 {
    60
}

/// Get recent metrics
async fn get_recent_metrics(
    State(state): State<Arc<ApiState>>,
    axum::extract::Query(query): axum::extract::Query<RecentMetricsQuery>,
) -> Json<Vec<RequestMetric>> {
    Json(state.metrics.get_recent(query.seconds))
}

/// Clear all metrics
async fn clear_metrics(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    state.metrics.clear();
    (StatusCode::OK, "Metrics cleared")
}

/// Test status response
#[derive(Debug, Serialize)]
pub struct TestStatusResponse {
    pub running: bool,
}

/// Get test status
async fn get_test_status(State(state): State<Arc<ApiState>>) -> Json<TestStatusResponse> {
    Json(TestStatusResponse {
        running: state.tester.is_running(),
    })
}

/// Run test request
#[derive(Debug, Deserialize)]
pub struct RunTestRequest {
    #[serde(flatten)]
    pub config: Option<TestConfig>,
}

/// Run API tests
async fn run_test(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<Option<RunTestRequest>>,
) -> impl IntoResponse {
    if state.tester.is_running() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "Test is already running"
            })),
        )
            .into_response();
    }

    let tester = state.tester.clone();
    let config = state.config.clone();

    // Get test config from request or use default
    let test_config = req
        .and_then(|r| r.config)
        .unwrap_or_else(|| config.get().test);

    // Run test in background and return immediately
    tokio::spawn(async move {
        match tester.run_with_config(test_config).await {
            Ok(summary) => {
                tracing::info!(
                    total = %summary.total_requests,
                    successful = %summary.successful,
                    "Test completed"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "Test failed");
            }
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Test started"
        })),
    )
        .into_response()
}

/// Stop running test
async fn stop_test(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    if !state.tester.is_running() {
        return (StatusCode::OK, "No test running");
    }

    state.tester.stop();
    (StatusCode::OK, "Test stopped")
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Health check endpoint
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response.status, "healthy");
    }
}
