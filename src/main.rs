//! API Check - Main Application
//!
//! A Rust application for HTTP request monitoring, proxy support, and API testing.

use api_check::{
    config::{AppConfig, SharedConfig},
    metrics::create_shared_metrics,
    server::start_server,
    testing::create_shared_tester,
    tui::TuiApp,
};
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// API Check - HTTP Server with Metrics Collection and API Testing
#[derive(Parser)]
#[command(name = "api-check")]
#[command(about = "A dev server with request counting, proxy support, and API testing")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Server host
    #[arg(long, env = "API_CHECK_SERVER_HOST")]
    host: Option<String>,

    /// Server port
    #[arg(short, long, env = "API_CHECK_SERVER_PORT")]
    port: Option<u16>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP server
    Server,
    /// Start the TUI dashboard
    Tui,
    /// Run API tests
    Test {
        /// Target URL to test
        #[arg(short, long)]
        target: Option<String>,
        /// Number of requests
        #[arg(short, long, default_value = "10")]
        num_calls: u32,
        /// Frequency in milliseconds
        #[arg(short, long, default_value = "100")]
        frequency: u64,
        /// HTTP method
        #[arg(short, long, default_value = "GET")]
        method: String,
    },
    /// Show current configuration
    Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("api_check={}", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let mut config = if std::path::Path::new(&cli.config).exists() {
        AppConfig::load_from_file(&cli.config).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to load config file, using defaults");
            AppConfig::default()
        })
    } else {
        AppConfig::load().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to load config, using defaults");
            AppConfig::default()
        })
    };

    // Override with CLI args
    if let Some(host) = cli.host {
        config.server.host = host;
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }

    let shared_config = SharedConfig::new(config.clone());
    let metrics = create_shared_metrics(10000);
    let tester = create_shared_tester(shared_config.clone(), metrics.clone());

    match cli.command {
        Some(Commands::Server) | None => {
            // Default: start the server
            tracing::info!(
                host = %config.server.host,
                port = %config.server.port,
                "Starting API Check server"
            );
            start_server(shared_config, metrics, tester).await?;
        }
        Some(Commands::Tui) => {
            // Start TUI with server in background
            let server_config = shared_config.clone();
            let server_metrics = metrics.clone();
            let server_tester = tester.clone();

            // Start server in background
            tokio::spawn(async move {
                if let Err(e) = start_server(server_config, server_metrics, server_tester).await {
                    tracing::error!(error = %e, "Server error");
                }
            });

            // Give server time to start
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Run TUI
            let mut app = TuiApp::new(shared_config, metrics, tester);
            app.run().await?;
        }
        Some(Commands::Test {
            target,
            num_calls,
            frequency,
            method,
        }) => {
            // Run API tests
            let mut test_config = config.test;
            test_config.num_calls = num_calls;
            test_config.frequency_ms = frequency;
            test_config.method = method;
            test_config.target_url = target;

            shared_config.update_test(test_config.clone());

            tracing::info!(
                target = %test_config.target_url.as_deref().unwrap_or("(default)"),
                num_calls = %num_calls,
                frequency = %frequency,
                "Running API tests"
            );

            let summary = tester.run_with_config(test_config).await?;

            println!("\n=== Test Results ===");
            println!("Total requests: {}", summary.total_requests);
            println!("Successful: {}", summary.successful);
            println!("Failed: {}", summary.failed);
            println!("Average latency: {:.2} ms", summary.avg_latency_ms);
            println!("Min latency: {:.2} ms", summary.min_latency_ms);
            println!("Max latency: {:.2} ms", summary.max_latency_ms);
            println!("Total duration: {:.2} ms", summary.total_duration_ms);
        }
        Some(Commands::Config) => {
            // Show current configuration
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }

    Ok(())
}
