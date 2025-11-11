mod api;
mod config;
mod core;
mod storage;

use anyhow::{Context, Result};
use api::{metrics::Metrics, rest_api, websocket};
use clap::Parser;
use config::AppConfig;
use core::{
    dedupe_engine::DeduplicationEngine, downstream::DownstreamForwarder, event_router::EventRouter,
    relay_pool::RelayPool,
};
use std::sync::Arc;
use std::time::Duration;
use storage::rocksdb_store::RocksDBStore;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(name = "iso-relayer")]
#[command(about = "ISO Relayer service", version)]
struct Cli {
    /// Path to configuration TOML file
    #[arg(long)]
    config: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // CLI
    let cli = Cli::parse();

    // Load config if provided
    let cfg: Option<AppConfig> = match &cli.config {
        Some(path) => Some(AppConfig::load_from_path(path)?),
        None => None,
    };

    // Initialize tracing - prefer config log level if provided, else env, else default
    let default_level = cfg
        .as_ref()
        .map(|c| c.monitoring.log_level.clone())
        .unwrap_or_else(|| "info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("iso_relayer={}", default_level).into()),
        )
        .init();

    info!("Starting ISO Relayer...");

    // Initialize metrics
    let metrics = Arc::new(Metrics::new().context("Failed to initialize metrics")?);

    // Initialize RocksDB storage
    let rocks_path = cfg
        .as_ref()
        .map(|c| c.deduplication.rocksdb_path.as_str())
        .unwrap_or("./data/rocksdb");
    let rocksdb =
        Arc::new(RocksDBStore::new(rocks_path).context("Failed to initialize RocksDB storage")?);
    info!("RocksDB storage initialized");

    // Initialize deduplication engine
    let dedupe_engine = match &cfg {
        Some(c) => Arc::new(
            DeduplicationEngine::new_with_params(
                rocksdb,
                c.deduplication.hotset_size,
                c.deduplication.bloom_capacity,
                c.deduplication.lru_size,
            )
            .with_metrics(metrics.clone()),
        ),
        None => Arc::new(DeduplicationEngine::new(rocksdb).with_metrics(metrics.clone())),
    };
    info!("Deduplication engine initialized");

    // Initialize relay pool
    let (health_check_interval, max_connections) = match &cfg {
        Some(c) => (
            Duration::from_secs(c.relay.health_check_interval),
            c.relay.max_connections,
        ),
        None => (Duration::from_secs(30), 10_000),
    };
    let (relay_pool, relay_event_rx) = RelayPool::new(health_check_interval, max_connections);
    let relay_pool = Arc::new(relay_pool.with_metrics(metrics.clone()));
    info!("Relay pool initialized");

    // Start health checks
    relay_pool.start_health_checks().await;
    info!("Health checks started");

    // Connect to relays (example - load from config file or environment)
    let relay_urls = match &cfg {
        Some(c) => c.relay.bootstrap_relays.clone(),
        None => load_relay_urls().await?,
    };
    info!("Loading {} relay URLs", relay_urls.len());

    relay_pool
        .subscribe_all(relay_urls)
        .await
        .context("Failed to subscribe to relays")?;
    info!("Subscribed to all relays");

    // Create downstream event channel
    let (downstream_tx, downstream_rx) = flume::unbounded();

    // Initialize event router
    let event_router = EventRouter::new(
        dedupe_engine.clone(),
        cfg.as_ref().map(|c| c.output.batch_size).unwrap_or(100), // batch size
        Duration::from_millis(cfg.as_ref().map(|c| c.output.max_latency_ms).unwrap_or(100) as u64), // max latency
        downstream_tx.clone(),
    )
    .with_metrics(metrics.clone());

    // Spawn event router task
    let router_handle = tokio::spawn(async move {
        if let Err(e) = event_router.process_stream(relay_event_rx).await {
            error!("Event router error: {}", e);
        }
    });

    // Create REST API router
    let rest_router = rest_api::create_router(relay_pool.clone(), metrics.clone());

    // Handle downstream forwarding based on config
    let websocket_enabled = cfg
        .as_ref()
        .map(|c| c.output.websocket_enabled)
        .unwrap_or(true);

    let app = if websocket_enabled {
        // Create WebSocket router (share the downstream event stream)
        let downstream_rx_arc = Arc::new(downstream_rx);
        let ws_router = websocket::create_websocket_router(downstream_rx_arc.clone());
        axum::Router::new().merge(rest_router).merge(ws_router)
    } else {
        // Forward events via TCP or HTTP instead of WebSocket
        let downstream_tcp = cfg
            .as_ref()
            .map(|c| c.output.downstream_tcp.clone())
            .unwrap_or_default();
        let downstream_rest = cfg
            .as_ref()
            .map(|c| c.output.downstream_rest.clone())
            .unwrap_or_default();

        if !downstream_tcp.is_empty() || !downstream_rest.is_empty() {
            let forwarder =
                DownstreamForwarder::new(downstream_tcp.clone(), downstream_rest.clone());
            let downstream_rx_for_forwarder = downstream_rx;
            tokio::spawn(async move {
                if let Err(e) = forwarder.forward_events(downstream_rx_for_forwarder).await {
                    error!("Downstream forwarder error: {}", e);
                }
            });
            info!(
                "Downstream forwarding enabled (TCP: {:?}, REST: {:?})",
                downstream_tcp, downstream_rest
            );
        } else {
            warn!(
                "websocket_enabled is false but no downstream endpoints configured. Events will be dropped."
            );
        }

        axum::Router::new().merge(rest_router)
    };

    // Start HTTP server
    let addr = match &cfg {
        Some(c) => format!("0.0.0.0:{}", c.output.websocket_port),
        None => "0.0.0.0:3000".to_string(),
    };
    info!("Starting HTTP server on {}", addr);
    let server_addr_for_logs = addr.clone();
    let server_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("Failed to bind to address")
            .unwrap();
        axum::serve(listener, app)
            .await
            .context("Failed to start server")
            .unwrap();
    });

    info!("ISO Relayer started successfully");
    info!("REST API: http://{}", server_addr_for_logs);
    info!("WebSocket: ws://{}/ws", server_addr_for_logs);
    info!("Metrics: http://{}/metrics", server_addr_for_logs);

    // Periodically update memory usage gauge
    {
        let metrics = metrics.clone();
        tokio::spawn(async move {
            use sysinfo::{ProcessesToUpdate, System};
            let mut sys = System::new();
            loop {
                sys.refresh_processes(ProcessesToUpdate::All, true);
                if let Ok(pid) = sysinfo::get_current_pid() {
                    if let Some(process) = sys.process(pid) {
                        // memory() returns bytes
                        metrics.memory_usage.set(process.memory() as f64 / 1024.0);
                    }
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
    // Wait for shutdown signal
    signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;
    info!("Shutdown signal received, gracefully shutting down...");

    // Cancel tasks
    router_handle.abort();
    server_handle.abort();

    info!("Shutdown complete");
    Ok(())
}

/// Load relay URLs from environment or config file
/// In production, this should load from a config file or database
async fn load_relay_urls() -> Result<Vec<String>> {
    // Example: load from environment variable
    if let Ok(urls) = std::env::var("RELAY_URLS") {
        return Ok(urls
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect());
    }

    // Default example relays (replace with actual relay URLs)
    Ok(vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.snort.social".to_string(),
    ])
}
