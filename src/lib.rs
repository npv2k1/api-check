//! API Check - HTTP Server with Metrics Collection and API Testing
//!
//! A Rust application providing:
//! - Dev server that receives HTTP requests
//! - Request counting and timing
//! - Proxy mode for forwarding requests
//! - Metrics collection and visualization
//! - API testing functionality

pub mod api;
pub mod config;
pub mod metrics;
pub mod proxy;
pub mod server;
pub mod testing;
pub mod tui;

pub use config::{AppConfig, SharedConfig};
pub use metrics::{create_shared_metrics, MetricsSummary, SharedMetrics};
pub use testing::{create_shared_tester, SharedTester};

/// Application result type
pub type Result<T> = anyhow::Result<T>;
