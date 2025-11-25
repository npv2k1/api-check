//! Basic usage example for api-check

use api_check::config::{AppConfig, SharedConfig, TestConfig};
use api_check::metrics::create_shared_metrics;
use api_check::testing::create_shared_tester;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = AppConfig::default();
    println!("Server configuration:");
    println!("  Host: {}", config.server.host);
    println!("  Port: {}", config.server.port);
    println!("  Proxy enabled: {}", config.proxy.enabled);

    // Create shared state
    let shared_config = SharedConfig::new(config);
    let metrics = create_shared_metrics(1000);
    let tester = create_shared_tester(shared_config.clone(), metrics.clone());

    // Update test configuration
    let test_config = TestConfig {
        num_calls: 5,
        frequency_ms: 100,
        method: "GET".to_string(),
        target_url: Some("https://httpbin.org/get".to_string()),
        body: None,
        headers: vec![],
    };
    shared_config.update_test(test_config.clone());

    println!("\nTest configuration:");
    println!("  Num calls: {}", test_config.num_calls);
    println!("  Frequency: {}ms", test_config.frequency_ms);
    println!("  Method: {}", test_config.method);
    println!("  Target: {:?}", test_config.target_url);

    // Run a simple test
    println!("\nRunning API test...");
    let summary = tester.run_with_config(test_config).await?;

    println!("\nTest Results:");
    println!("  Total requests: {}", summary.total_requests);
    println!("  Successful: {}", summary.successful);
    println!("  Failed: {}", summary.failed);
    println!("  Avg latency: {:.2}ms", summary.avg_latency_ms);
    println!("  Min latency: {:.2}ms", summary.min_latency_ms);
    println!("  Max latency: {:.2}ms", summary.max_latency_ms);

    // Check metrics
    let metrics_summary = metrics.get_summary();
    println!("\nMetrics Summary:");
    println!("  Total recorded: {}", metrics_summary.total_requests);
    println!("  Requests/sec: {:.2}", metrics_summary.requests_per_second);

    Ok(())
}
