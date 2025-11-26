//! TUI module for realtime metrics visualization
//!
//! Provides a terminal user interface with realtime charts for metrics.

use crate::config::SharedConfig;
use crate::metrics::SharedMetrics;
use crate::testing::SharedTester;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Wrap,
    },
    Frame, Terminal,
};
use std::io;
use tokio::time::Duration;

/// TUI Application state
pub struct TuiApp {
    config: SharedConfig,
    metrics: SharedMetrics,
    tester: SharedTester,
    should_quit: bool,
    /// Latency history for sparkline chart
    latency_history: Vec<u64>,
    /// Request count history
    request_history: Vec<u64>,
    /// Last known request count
    last_request_count: usize,
    /// Status message
    status_message: String,
}

impl TuiApp {
    /// Create a new TUI application
    pub fn new(config: SharedConfig, metrics: SharedMetrics, tester: SharedTester) -> Self {
        Self {
            config,
            metrics,
            tester,
            should_quit: false,
            latency_history: Vec::with_capacity(100),
            request_history: Vec::with_capacity(100),
            last_request_count: 0,
            status_message: "Press 'h' for help, 'q' to quit".to_string(),
        }
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Update data
            self.update_data();

            // Draw UI
            terminal.draw(|f| self.ui(f))?;

            // Handle input with timeout for realtime updates
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('h') => {
                                self.status_message = "q=quit, t=run test, s=stop test, c=clear metrics, p=toggle proxy".to_string();
                            }
                            KeyCode::Char('t') => {
                                if self.tester.is_running() {
                                    self.status_message = "Test already running".to_string();
                                } else {
                                    let tester = self.tester.clone();
                                    tokio::spawn(async move {
                                        let _ = tester.run().await;
                                    });
                                    self.status_message = "Test started".to_string();
                                }
                            }
                            KeyCode::Char('s') => {
                                if self.tester.is_running() {
                                    self.tester.stop();
                                    self.status_message = "Test stopped".to_string();
                                } else {
                                    self.status_message = "No test running".to_string();
                                }
                            }
                            KeyCode::Char('c') => {
                                self.metrics.clear();
                                self.latency_history.clear();
                                self.request_history.clear();
                                self.last_request_count = 0;
                                self.status_message = "Metrics cleared".to_string();
                            }
                            KeyCode::Char('p') => {
                                let mut config = self.config.get();
                                config.proxy.enabled = !config.proxy.enabled;
                                let enabled = config.proxy.enabled;
                                self.config.update(config);
                                self.status_message = format!(
                                    "Proxy {}",
                                    if enabled { "enabled" } else { "disabled" }
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Update metrics data for charts
    fn update_data(&mut self) {
        let summary = self.metrics.get_summary();

        // Update latency history (convert to u64 for sparkline)
        if summary.total_requests > 0 {
            // Safely convert f64 to u64, clamping to valid range
            let avg_latency = summary.avg_latency_ms.max(0.0).round() as u64;
            self.latency_history.push(avg_latency);
            if self.latency_history.len() > 100 {
                self.latency_history.remove(0);
            }
        }

        // Update request history (new requests since last update)
        let current_count = self.metrics.count();
        let new_requests = current_count.saturating_sub(self.last_request_count) as u64;
        self.request_history.push(new_requests);
        if self.request_history.len() > 100 {
            self.request_history.remove(0);
        }
        self.last_request_count = current_count;
    }

    /// Draw the UI
    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(8), // Summary stats
                Constraint::Length(8), // Sparkline charts
                Constraint::Min(10),   // Status distribution
                Constraint::Length(3), // Status bar
            ])
            .split(f.size());

        self.draw_header(f, chunks[0]);
        self.draw_summary(f, chunks[1]);
        self.draw_charts(f, chunks[2]);
        self.draw_status_distribution(f, chunks[3]);
        self.draw_status_bar(f, chunks[4]);
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let config = self.config.get();
        let proxy_status = if config.proxy.enabled {
            format!(
                "Proxy: ON -> {}",
                config.proxy.target.as_deref().unwrap_or("(no target)")
            )
        } else {
            "Proxy: OFF".to_string()
        };

        let title = format!(
            " API Check - {}:{} | {} ",
            config.server.host, config.server.port, proxy_status
        );

        let header = Paragraph::new(title)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(header, area);
    }

    fn draw_summary(&self, f: &mut Frame, area: Rect) {
        let summary = self.metrics.get_summary();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        // Total requests
        let total = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{}", summary.total_requests),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Total Requests",
                Style::default().fg(Color::Gray),
            )),
        ])
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Total"));
        f.render_widget(total, chunks[0]);

        // Success rate
        let success_rate = if summary.total_requests > 0 {
            (summary.successful_requests as f64 / summary.total_requests as f64 * 100.0) as u16
        } else {
            0
        };
        let success = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Success Rate"))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(success_rate as f64 / 100.0)
            .label(format!("{}%", success_rate));
        f.render_widget(success, chunks[1]);

        // Average latency
        let latency = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{:.2} ms", summary.avg_latency_ms),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Avg Latency",
                Style::default().fg(Color::Gray),
            )),
        ])
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Latency"));
        f.render_widget(latency, chunks[2]);

        // RPS
        let rps = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{:.2}", summary.requests_per_second),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled("Req/sec", Style::default().fg(Color::Gray))),
        ])
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("RPS"));
        f.render_widget(rps, chunks[3]);
    }

    fn draw_charts(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Latency sparkline
        let latency_data: Vec<u64> = self.latency_history.clone();
        let latency_sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Latency History (ms)"),
            )
            .data(&latency_data)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(latency_sparkline, chunks[0]);

        // Request rate sparkline
        let request_data: Vec<u64> = self.request_history.clone();
        let request_sparkline = Sparkline::default()
            .block(Block::default().borders(Borders::ALL).title("Request Rate"))
            .data(&request_data)
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(request_sparkline, chunks[1]);
    }

    fn draw_status_distribution(&self, f: &mut Frame, area: Rect) {
        let summary = self.metrics.get_summary();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Status code bar chart
        let mut status_groups: Vec<(String, u64, Color)> = summary
            .status_distribution
            .iter()
            .map(|(code, count)| {
                let color = match *code {
                    200..=299 => Color::Green,
                    300..=399 => Color::Yellow,
                    400..=499 => Color::Red,
                    500..=599 => Color::Magenta,
                    _ => Color::Gray,
                };
                (format!("{}", code), *count, color)
            })
            .collect();
        status_groups.sort_by_key(|(code, _, _)| code.clone());

        // Create bars for the bar chart
        let bars: Vec<Bar> = status_groups
            .iter()
            .map(|(label, value, color)| {
                Bar::default()
                    .value(*value)
                    .label(Line::from(label.clone()))
                    .style(Style::default().fg(*color))
            })
            .collect();

        let bar_chart = BarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Status Codes"))
            .data(BarGroup::default().bars(&bars))
            .bar_width(5)
            .bar_gap(1);
        f.render_widget(bar_chart, chunks[0]);

        // Recent requests list
        let recent = self.metrics.get_recent(60);
        let items: Vec<ListItem> = recent
            .iter()
            .rev()
            .take(10)
            .map(|m| {
                let status_color = match m.status_code {
                    Some(200..=299) => Color::Green,
                    Some(300..=399) => Color::Yellow,
                    Some(400..=499) => Color::Red,
                    Some(500..=599) => Color::Magenta,
                    _ => Color::Gray,
                };
                let status = m.status_code.map_or("-".to_string(), |s| s.to_string());
                let text = format!("{} {} [{}] {:.1}ms", m.method, m.path, status, m.latency_ms);
                ListItem::new(text).style(Style::default().fg(status_color))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Recent Requests"),
        );
        f.render_widget(list, chunks[1]);
    }

    fn draw_status_bar(&self, f: &mut Frame, area: Rect) {
        let test_status = if self.tester.is_running() {
            "Test: RUNNING"
        } else {
            "Test: IDLE"
        };

        let status = Paragraph::new(format!("{} | {}", self.status_message, test_status))
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Status"));
        f.render_widget(status, area);
    }
}
