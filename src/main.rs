use clap::Parser;
use colored::Colorize;
use std::sync::Arc;
use tokio::sync::RwLock;

mod config;
mod database;
mod miner;
mod pool;
mod proxy;
mod api;
mod metrics;
mod logger;

use config::Config;
use database::Database;
use miner::MinerManager;
use pool::PoolManager;
use metrics::SystemMetrics;

const VERSION: &str = "3.4";

#[derive(Parser, Debug)]
#[command(name = "tunnel")]
#[command(about = "Mining Pool Proxy", long_about = None)]
struct Args {
    /// Disable database logging
    #[arg(long)]
    nodata: bool,

    /// Disable API server
    #[arg(long)]
    noapi: bool,

    /// Minimal output mode (single line status)
    #[arg(long)]
    nodebug: bool,

    /// Enable TLS support for miner connections
    #[arg(long)]
    tls: bool,

    /// TLS certificate file
    #[arg(long, default_value = "cert.pem")]
    tlscert: String,

    /// TLS key file
    #[arg(long, default_value = "key.pem")]
    tlskey: String,

    /// Show version
    #[arg(long)]
    version: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("{}", format!("Tunnel v{} by mytai", VERSION).bright_cyan());
    println!("{}", "-".repeat(60));

    if args.version {
        println!("Tunnel v{}", VERSION);
        return Ok(());
    }

    // Initialize logger
    if !args.nodebug {
        tracing_subscriber::fmt::init();
    }

    // Load configuration
    let config = Config::load_or_create("config.yml").await?;

    if !args.nodebug {
        println!("{}", format!("Loaded {} pools", config.pools.len()).green());
    }

    // Initialize database
    let database = if !args.nodata {
        let db = Database::new("./data.db", "./system.db").await?;
        if !args.nodebug {
            println!("{}", "Database connected (Pure Rust SQLite)".green());
        }
        Some(Arc::new(db))
    } else {
        None
    };

    // Initialize managers
    let miner_manager = Arc::new(MinerManager::new());
    let pool_manager = Arc::new(PoolManager::new());
    let system_metrics = Arc::new(RwLock::new(SystemMetrics::new().await));

    // Start system metrics updater
    let metrics_clone = Arc::clone(&system_metrics);
    let miner_clone = Arc::clone(&miner_manager);
    tokio::spawn(async move {
        metrics::update_system_metrics(metrics_clone, miner_clone).await;
    });

    // Start pool ping monitor
    let pool_clone = Arc::clone(&pool_manager);
    let config_clone = config.clone();
    tokio::spawn(async move {
        pool::monitor_pool_pings(pool_clone, config_clone).await;
    });

    // Start tunnels
    for (name, tunnel_config) in &config.tunnels {
        let pool_config = config.pools.get(&tunnel_config.pool)
            .ok_or_else(|| anyhow::anyhow!("Pool {} not found", tunnel_config.pool))?;

        let miner_mgr = Arc::clone(&miner_manager);
        let pool_mgr = Arc::clone(&pool_manager);
        let db = database.clone();
        let tname = name.clone();
        let tconfig = tunnel_config.clone();
        let pconfig = pool_config.clone();
        let tls_enabled = args.tls;
        let cert_file = args.tlscert.clone();
        let key_file = args.tlskey.clone();
        let nodebug = args.nodebug;

        tokio::spawn(async move {
            if let Err(e) = proxy::start_tunnel(
                &tname,
                tconfig,
                pconfig,
                miner_mgr,
                pool_mgr,
                db,
                tls_enabled,
                &cert_file,
                &key_file,
                nodebug,
            ).await {
                eprintln!("{}", format!("Tunnel {} error: {}", tname, e).red());
            }
        });
    }

    // Start API server
    if !args.noapi {
        let api_port = config.api_port;
        let miner_mgr = Arc::clone(&miner_manager);
        let pool_mgr = Arc::clone(&pool_manager);
        let sys_metrics = Arc::clone(&system_metrics);
        let db = database.clone();

        tokio::spawn(async move {
            if let Err(e) = api::start_api_server(
                api_port,
                miner_mgr,
                pool_mgr,
                sys_metrics,
                db,
            ).await {
                eprintln!("{}", format!("API server error: {}", e).red());
            }
        });

        if !args.nodebug {
            println!("{}", format!("API server running on port {}", config.api_port).green());
        }
    }

    if !args.nodebug {
        println!("{}", format!("Tunnel Started").green());
        println!("{}", format!("Active tunnels: {}", config.tunnels.len()).green());
        if args.tls {
            println!("{}", "TLS support enabled".green());
        }
        if args.nodata {
            println!("{}", "Database logging disabled".yellow());
        }
        println!("{}", "-".repeat(60));
    }

    // Keep running
    tokio::signal::ctrl_c().await?;
    println!("\n{}", "Shutting down...".yellow());

    Ok(())
}
