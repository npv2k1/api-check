//! Metrics collection module
//!
//! Collects and stores metrics about requests, latency, and status codes.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A single request metric entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetric {
    /// Unique request ID
    pub id: String,
    /// HTTP method
    pub method: String,
    /// Request path
    pub path: String,
    /// Response status code (if available)
    pub status_code: Option<u16>,
    /// Request processing time in milliseconds
    pub latency_ms: f64,
    /// Timestamp when request was received
    pub timestamp: DateTime<Utc>,
    /// Whether this was a proxied request
    pub proxied: bool,
}

impl RequestMetric {
    /// Create a new request metric
    pub fn new(method: String, path: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method,
            path,
            status_code: None,
            latency_ms: 0.0,
            timestamp: Utc::now(),
            proxied: false,
        }
    }

    /// Set the status code
    pub fn with_status(mut self, status: u16) -> Self {
        self.status_code = Some(status);
        self
    }

    /// Set the latency
    pub fn with_latency(mut self, latency_ms: f64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Mark as proxied
    pub fn with_proxied(mut self, proxied: bool) -> Self {
        self.proxied = proxied;
        self
    }
}

/// Aggregated metrics summary
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsSummary {
    /// Total number of requests
    pub total_requests: u64,
    /// Number of successful requests (2xx status)
    pub successful_requests: u64,
    /// Number of failed requests (4xx, 5xx status)
    pub failed_requests: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Minimum latency in milliseconds
    pub min_latency_ms: f64,
    /// Maximum latency in milliseconds
    pub max_latency_ms: f64,
    /// Number of proxied requests
    pub proxied_requests: u64,
    /// Status code distribution
    pub status_distribution: HashMap<u16, u64>,
    /// Requests per second (over last minute)
    pub requests_per_second: f64,
}

/// Metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    /// All recorded metrics
    metrics: RwLock<Vec<RequestMetric>>,
    /// Maximum number of metrics to keep in memory
    max_entries: usize,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(max_entries: usize) -> Self {
        Self {
            metrics: RwLock::new(Vec::with_capacity(max_entries)),
            max_entries,
        }
    }

    /// Record a new request metric
    pub fn record(&self, metric: RequestMetric) {
        let mut metrics = self.metrics.write();
        if metrics.len() >= self.max_entries {
            // Remove oldest entries when at capacity
            metrics.drain(0..self.max_entries / 10);
        }
        metrics.push(metric);
    }

    /// Get all metrics
    pub fn get_all(&self) -> Vec<RequestMetric> {
        self.metrics.read().clone()
    }

    /// Get metrics from the last N seconds
    pub fn get_recent(&self, seconds: i64) -> Vec<RequestMetric> {
        let cutoff = Utc::now() - chrono::Duration::seconds(seconds);
        self.metrics
            .read()
            .iter()
            .filter(|m| m.timestamp > cutoff)
            .cloned()
            .collect()
    }

    /// Get aggregated summary
    pub fn get_summary(&self) -> MetricsSummary {
        let metrics = self.metrics.read();

        if metrics.is_empty() {
            return MetricsSummary::default();
        }

        let total_requests = metrics.len() as u64;
        let mut successful_requests = 0u64;
        let mut failed_requests = 0u64;
        let mut total_latency = 0.0;
        let mut min_latency = f64::MAX;
        let mut max_latency = 0.0f64;
        let mut proxied_requests = 0u64;
        let mut status_distribution = HashMap::new();

        for metric in metrics.iter() {
            total_latency += metric.latency_ms;
            min_latency = min_latency.min(metric.latency_ms);
            max_latency = max_latency.max(metric.latency_ms);

            if metric.proxied {
                proxied_requests += 1;
            }

            if let Some(status) = metric.status_code {
                *status_distribution.entry(status).or_insert(0) += 1;
                if (200..300).contains(&status) {
                    successful_requests += 1;
                } else if status >= 400 {
                    failed_requests += 1;
                }
            }
        }

        // Calculate requests per second over last minute
        let one_minute_ago = Utc::now() - chrono::Duration::minutes(1);
        let recent_count = metrics
            .iter()
            .filter(|m| m.timestamp > one_minute_ago)
            .count() as f64;
        let requests_per_second = recent_count / 60.0;

        MetricsSummary {
            total_requests,
            successful_requests,
            failed_requests,
            avg_latency_ms: total_latency / total_requests as f64,
            min_latency_ms: if min_latency == f64::MAX {
                0.0
            } else {
                min_latency
            },
            max_latency_ms: max_latency,
            proxied_requests,
            status_distribution,
            requests_per_second,
        }
    }

    /// Clear all metrics
    pub fn clear(&self) {
        self.metrics.write().clear();
    }

    /// Get the count of requests
    pub fn count(&self) -> usize {
        self.metrics.read().len()
    }

    /// Get latency histogram data for charts
    pub fn get_latency_histogram(&self, buckets: usize) -> Vec<(f64, u64)> {
        let metrics = self.metrics.read();

        if metrics.is_empty() {
            return vec![];
        }

        let min_latency = metrics
            .iter()
            .map(|m| m.latency_ms)
            .fold(f64::MAX, f64::min);
        let max_latency = metrics.iter().map(|m| m.latency_ms).fold(0.0, f64::max);

        // Handle edge cases: same values, invalid ranges, or insufficient data
        if max_latency == min_latency || buckets == 0 {
            return vec![(min_latency, metrics.len() as u64)];
        }

        let bucket_size = (max_latency - min_latency) / buckets as f64;
        // Guard against NaN or zero bucket_size
        if !bucket_size.is_finite() || bucket_size == 0.0 {
            return vec![(min_latency, metrics.len() as u64)];
        }
        let mut histogram = vec![0u64; buckets];

        for metric in metrics.iter() {
            let bucket = ((metric.latency_ms - min_latency) / bucket_size) as usize;
            let bucket = bucket.min(buckets - 1);
            histogram[bucket] += 1;
        }

        histogram
            .into_iter()
            .enumerate()
            .map(|(i, count)| (min_latency + (i as f64 * bucket_size), count))
            .collect()
    }

    /// Get time-series data for realtime charts
    pub fn get_time_series(&self, points: usize) -> Vec<(DateTime<Utc>, f64)> {
        let metrics = self.metrics.read();

        if metrics.is_empty() {
            return vec![];
        }

        let recent: Vec<_> = metrics.iter().rev().take(points).collect();
        recent
            .into_iter()
            .rev()
            .map(|m| (m.timestamp, m.latency_ms))
            .collect()
    }
}

/// Shared metrics collector for use across threads
pub type SharedMetrics = Arc<MetricsCollector>;

/// Create a new shared metrics collector
pub fn create_shared_metrics(max_entries: usize) -> SharedMetrics {
    Arc::new(MetricsCollector::new(max_entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_metric() {
        let collector = MetricsCollector::new(100);
        let metric = RequestMetric::new("GET".to_string(), "/test".to_string())
            .with_status(200)
            .with_latency(10.5);

        collector.record(metric);
        assert_eq!(collector.count(), 1);
    }

    #[test]
    fn test_summary() {
        let collector = MetricsCollector::new(100);

        for i in 0..10 {
            let metric = RequestMetric::new("GET".to_string(), "/test".to_string())
                .with_status(200)
                .with_latency((i * 10) as f64);
            collector.record(metric);
        }

        let summary = collector.get_summary();
        assert_eq!(summary.total_requests, 10);
        assert_eq!(summary.successful_requests, 10);
        assert_eq!(summary.min_latency_ms, 0.0);
        assert_eq!(summary.max_latency_ms, 90.0);
    }

    #[test]
    fn test_max_entries() {
        let collector = MetricsCollector::new(20);

        for i in 0..30 {
            let metric = RequestMetric::new("GET".to_string(), format!("/test/{}", i));
            collector.record(metric);
        }

        // Should have removed some entries
        assert!(collector.count() < 30);
    }
}
