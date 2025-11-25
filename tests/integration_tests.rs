//! Integration tests for api-check

use api_check::config::{AppConfig, ProxyConfig, SharedConfig, TestConfig};
use api_check::metrics::{create_shared_metrics, RequestMetric};

#[test]
fn test_config_default() {
    let config = AppConfig::default();
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 3000);
    assert!(!config.proxy.enabled);
}

#[test]
fn test_shared_config_update() {
    let config = AppConfig::default();
    let shared = SharedConfig::new(config);

    let proxy = ProxyConfig {
        enabled: true,
        target: Some("http://example.com".to_string()),
    };
    shared.update_proxy(proxy);

    let updated = shared.get();
    assert!(updated.proxy.enabled);
    assert_eq!(updated.proxy.target, Some("http://example.com".to_string()));
}

#[test]
fn test_metrics_recording() {
    let metrics = create_shared_metrics(100);

    let metric = RequestMetric::new("GET".to_string(), "/test".to_string())
        .with_status(200)
        .with_latency(10.5);

    metrics.record(metric);

    assert_eq!(metrics.count(), 1);

    let summary = metrics.get_summary();
    assert_eq!(summary.total_requests, 1);
    assert_eq!(summary.successful_requests, 1);
}

#[test]
fn test_metrics_summary() {
    let metrics = create_shared_metrics(100);

    // Record multiple metrics
    for i in 0..5 {
        let status = if i % 2 == 0 { 200 } else { 500 };
        let metric = RequestMetric::new("GET".to_string(), format!("/test/{}", i))
            .with_status(status)
            .with_latency((i * 10) as f64);
        metrics.record(metric);
    }

    let summary = metrics.get_summary();
    assert_eq!(summary.total_requests, 5);
    assert_eq!(summary.successful_requests, 3); // 200 status codes
    assert_eq!(summary.failed_requests, 2); // 500 status codes
}

#[test]
fn test_test_config_default() {
    let config = TestConfig::default();
    assert_eq!(config.num_calls, 10);
    assert_eq!(config.frequency_ms, 100);
    assert_eq!(config.method, "GET");
}

#[test]
fn test_metrics_clear() {
    let metrics = create_shared_metrics(100);

    for _ in 0..5 {
        let metric = RequestMetric::new("GET".to_string(), "/test".to_string());
        metrics.record(metric);
    }

    assert_eq!(metrics.count(), 5);

    metrics.clear();

    assert_eq!(metrics.count(), 0);
}
