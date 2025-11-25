//! API Testing module
//!
//! Provides functionality to test APIs with configurable parameters.

use crate::config::{SharedConfig, TestConfig};
use crate::metrics::{RequestMetric, SharedMetrics};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test result for a single API call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Request index (1-based)
    pub index: u32,
    /// Whether the request was successful
    pub success: bool,
    /// Response status code (if received)
    pub status_code: Option<u16>,
    /// Latency in milliseconds
    pub latency_ms: f64,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Aggregated test run results
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TestRunSummary {
    /// Total number of requests made
    pub total_requests: u32,
    /// Number of successful requests
    pub successful: u32,
    /// Number of failed requests
    pub failed: u32,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Minimum latency
    pub min_latency_ms: f64,
    /// Maximum latency
    pub max_latency_ms: f64,
    /// Total test duration in milliseconds
    pub total_duration_ms: f64,
    /// Individual test results
    pub results: Vec<TestResult>,
}

/// API Tester
pub struct ApiTester {
    client: Client,
    config: SharedConfig,
    metrics: SharedMetrics,
    running: Arc<AtomicBool>,
}

impl ApiTester {
    /// Create a new API tester
    pub fn new(config: SharedConfig, metrics: SharedMetrics) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            metrics,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if a test is currently running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the current test run
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Run API tests with the current configuration
    pub async fn run(&self) -> Result<TestRunSummary> {
        self.run_with_config(self.config.get().test).await
    }

    /// Run API tests with a custom configuration
    pub async fn run_with_config(&self, test_config: TestConfig) -> Result<TestRunSummary> {
        if self.running.swap(true, Ordering::Relaxed) {
            anyhow::bail!("Test is already running");
        }

        let start = Instant::now();
        let mut results = Vec::with_capacity(test_config.num_calls as usize);

        // Determine target URL
        let app_config = self.config.get();
        let target_url = test_config.target_url.clone().unwrap_or_else(|| {
            format!(
                "http://{}:{}/",
                app_config.server.host, app_config.server.port
            )
        });

        let method: reqwest::Method = test_config.method.parse().unwrap_or(reqwest::Method::GET);

        tracing::info!(
            target = %target_url,
            method = %method,
            num_calls = %test_config.num_calls,
            frequency_ms = %test_config.frequency_ms,
            "Starting API test"
        );

        for i in 0..test_config.num_calls {
            if !self.running.load(Ordering::Relaxed) {
                tracing::info!("Test stopped by user");
                break;
            }

            let result = self
                .make_request(&target_url, method.clone(), &test_config)
                .await;

            let test_result = match result {
                Ok((status, latency)) => {
                    // Record metric
                    let metric = RequestMetric::new(method.to_string(), target_url.clone())
                        .with_status(status)
                        .with_latency(latency);
                    self.metrics.record(metric);

                    TestResult {
                        index: i + 1,
                        success: (200..300).contains(&status),
                        status_code: Some(status),
                        latency_ms: latency,
                        error: None,
                    }
                }
                Err(e) => {
                    let latency = 0.0;
                    let metric = RequestMetric::new(method.to_string(), target_url.clone())
                        .with_latency(latency);
                    self.metrics.record(metric);

                    TestResult {
                        index: i + 1,
                        success: false,
                        status_code: None,
                        latency_ms: latency,
                        error: Some(e.to_string()),
                    }
                }
            };

            results.push(test_result);

            // Wait between requests (unless it's the last one)
            if i < test_config.num_calls - 1 && test_config.frequency_ms > 0 {
                tokio::time::sleep(Duration::from_millis(test_config.frequency_ms)).await;
            }
        }

        self.running.store(false, Ordering::Relaxed);

        // Calculate summary
        let total_requests = results.len() as u32;
        let successful = results.iter().filter(|r| r.success).count() as u32;
        let failed = total_requests - successful;

        let latencies: Vec<f64> = results.iter().map(|r| r.latency_ms).collect();
        let avg_latency_ms = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        };
        let min_latency_ms = latencies.iter().cloned().fold(f64::MAX, f64::min);
        let max_latency_ms = latencies.iter().cloned().fold(0.0, f64::max);

        let summary = TestRunSummary {
            total_requests,
            successful,
            failed,
            avg_latency_ms,
            min_latency_ms: if min_latency_ms == f64::MAX {
                0.0
            } else {
                min_latency_ms
            },
            max_latency_ms,
            total_duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            results,
        };

        tracing::info!(
            total = %total_requests,
            successful = %successful,
            failed = %failed,
            avg_latency = %avg_latency_ms,
            "Test completed"
        );

        Ok(summary)
    }

    /// Make a single HTTP request
    async fn make_request(
        &self,
        url: &str,
        method: reqwest::Method,
        config: &TestConfig,
    ) -> Result<(u16, f64)> {
        let start = Instant::now();

        let mut builder = self.client.request(method, url);

        // Add custom headers
        for (key, value) in &config.headers {
            builder = builder.header(key, value);
        }

        // Add body for POST/PUT requests
        if let Some(body) = &config.body {
            builder = builder.body(body.clone());
            builder = builder.header("Content-Type", "application/json");
        }

        let response = builder.send().await?;
        let status = response.status().as_u16();
        let latency = start.elapsed().as_secs_f64() * 1000.0;

        Ok((status, latency))
    }
}

/// Shared API tester
pub type SharedTester = Arc<ApiTester>;

/// Create a shared API tester
pub fn create_shared_tester(config: SharedConfig, metrics: SharedMetrics) -> SharedTester {
    Arc::new(ApiTester::new(config, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::metrics::create_shared_metrics;

    #[test]
    fn test_tester_creation() {
        let config = SharedConfig::new(AppConfig::default());
        let metrics = create_shared_metrics(1000);
        let tester = ApiTester::new(config, metrics);

        assert!(!tester.is_running());
    }

    #[test]
    fn test_result_serialization() {
        let result = TestResult {
            index: 1,
            success: true,
            status_code: Some(200),
            latency_ms: 10.5,
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
