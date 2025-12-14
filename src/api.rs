use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

use crate::{
    database::Database,
    miner::{MinerManager, MinerInfo},
    pool::PoolManager,
    metrics::SystemMetrics,
};

pub type AppState = Arc<ApiState>;

pub struct ApiState {
    pub miner_manager: Arc<MinerManager>,
    pub pool_manager: Arc<PoolManager>,
    pub system_metrics: Arc<RwLock<SystemMetrics>>,
    pub database: Option<Arc<Database>>,
}

pub async fn start_api_server(
    port: u16,
    miner_manager: Arc<MinerManager>,
    pool_manager: Arc<PoolManager>,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    database: Option<Arc<Database>>,
) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        miner_manager,
        pool_manager,
        system_metrics,
        database,
    });

    let app = Router::new()
        .route("/api/metrics", get(handle_metrics))
        .route("/api/i/:wallet", get(handle_miner_info))
        .route("/api/network/stats", get(handle_network_stats))
        .route("/api/shares/stats", get(handle_shares_stats))
        .route("/metrics", get(handle_prometheus_metrics))
        .route("/api/logs/stream", get(handle_websocket))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Serialize)]
struct MetricsResponse {
    system: SystemInfo,
    database: DatabaseInfo,
    network: NetworkInfo,
    miners: MinersInfo,
    pools: serde_json::Value,
}

#[derive(Serialize)]
struct SystemInfo {
    cpu_model: String,
    cpu_cores: usize,
    cpu_usage_percent: String,
    ram_total_bytes: u64,
    ram_used_bytes: u64,
    ram_usage_percent: String,
    disk_total_bytes: u64,
    disk_used_bytes: u64,
    disk_usage_percent: String,
    os: String,
    public_ip: String,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct DatabaseInfo {
    data_db_size_bytes: u64,
    data_db_size_mb: f64,
    system_db_size_bytes: u64,
    system_db_size_mb: f64,
}

#[derive(Serialize)]
struct NetworkInfo {
    total_download_bytes: i64,
    total_download_mb: f64,
    total_download_gb: f64,
    total_upload_bytes: i64,
    total_upload_mb: f64,
    total_upload_gb: f64,
    packets_sent: i64,
    packets_received: i64,
}

#[derive(Serialize)]
struct MinersInfo {
    active_count: usize,
    list: Vec<MinerData>,
}

#[derive(Serialize)]
struct MinerData {
    wallet: String,
    name: String,
    ip: String,
    pool: String,
    shares_accepted: i64,
    shares_rejected: i64,
    current_hashrate: String,
    average_hashrate: String,
    difficulty: f64,
    uptime_seconds: i64,
}

async fn handle_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let metrics = state.system_metrics.read().await;
    let pools = state.pool_manager.get_all_pools().await;
    let miners = state.miner_manager.get_all_miners().await;

    let mut pools_data = serde_json::Map::new();
    for pool_arc in pools {
        let pool = pool_arc.read().await;
        let pool_info = serde_json::json!({
            "current_ping_ms": pool.current_ping,
            "average_ping_ms": pool.average_ping,
            "avg_accept_time_ms": pool.avg_accept_time,
            "shares_accepted": pool.shares_accepted,
            "shares_rejected": pool.shares_rejected,
            "last_ping_time": pool.last_ping_time.to_rfc3339(),
        });
        pools_data.insert(pool.name.clone(), pool_info);
    }

    let mut miners_list = Vec::new();
    for miner_arc in miners {
        let miner = miner_arc.read().await;
        let uptime = (chrono::Utc::now() - miner.connected_at).num_seconds();

        miners_list.push(MinerData {
            wallet: miner.wallet.clone(),
            name: miner.name.clone(),
            ip: miner.ip.clone(),
            pool: miner.pool_name.clone(),
            shares_accepted: miner.shares_accepted.load(std::sync::atomic::Ordering::Relaxed),
            shares_rejected: miner.shares_rejected.load(std::sync::atomic::Ordering::Relaxed),
            current_hashrate: MinerInfo::format_hashrate(miner.current_hashrate),
            average_hashrate: MinerInfo::format_hashrate(miner.average_hashrate),
            difficulty: miner.difficulty,
            uptime_seconds: uptime,
        });
    }

    let data_db_size = get_file_size("./data.db");
    let system_db_size = get_file_size("./system.db");

    let mut total_download = 0i64;
    let mut total_upload = 0i64;
    let mut total_sent = 0i64;
    let mut total_received = 0i64;

    for miner_arc in state.miner_manager.get_all_miners().await {
        let miner = miner_arc.read().await;
        total_download += miner.bytes_download.load(std::sync::atomic::Ordering::Relaxed);
        total_upload += miner.bytes_upload.load(std::sync::atomic::Ordering::Relaxed);
        total_sent += miner.packets_sent.load(std::sync::atomic::Ordering::Relaxed);
        total_received += miner.packets_received.load(std::sync::atomic::Ordering::Relaxed);
    }

    let response = MetricsResponse {
        system: SystemInfo {
            cpu_model: metrics.cpu_model.clone(),
            cpu_cores: metrics.cpu_cores,
            cpu_usage_percent: format!("{:.2}%", metrics.cpu_usage),
            ram_total_bytes: metrics.ram_total,
            ram_used_bytes: metrics.ram_used,
            ram_usage_percent: format!("{:.2}%", (metrics.ram_used as f64 / metrics.ram_total as f64) * 100.0),
            disk_total_bytes: metrics.disk_total,
            disk_used_bytes: metrics.disk_used,
            disk_usage_percent: format!("{:.2}%", (metrics.disk_used as f64 / metrics.disk_total as f64) * 100.0),
            os: metrics.os.clone(),
            public_ip: metrics.public_ip.clone(),
            uptime_seconds: metrics.uptime.as_secs(),
        },
        database: DatabaseInfo {
            data_db_size_bytes: data_db_size,
            data_db_size_mb: data_db_size as f64 / 1024.0 / 1024.0,
            system_db_size_bytes: system_db_size,
            system_db_size_mb: system_db_size as f64 / 1024.0 / 1024.0,
        },
        network: NetworkInfo {
            total_download_bytes: total_download,
            total_download_mb: total_download as f64 / 1024.0 / 1024.0,
            total_download_gb: total_download as f64 / 1024.0 / 1024.0 / 1024.0,
            total_upload_bytes: total_upload,
            total_upload_mb: total_upload as f64 / 1024.0 / 1024.0,
            total_upload_gb: total_upload as f64 / 1024.0 / 1024.0 / 1024.0,
            packets_sent: total_sent,
            packets_received: total_received,
        },
        miners: MinersInfo {
            active_count: metrics.active_miners,
            list: miners_list,
        },
        pools: serde_json::Value::Object(pools_data),
    };

    Json(response)
}

async fn handle_miner_info(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut active_miner = None;
    let miners = state.miner_manager.get_all_miners().await;

    for miner_arc in miners {
        let miner = miner_arc.read().await;
        if miner.wallet.starts_with(&wallet) {
            let uptime = (chrono::Utc::now() - miner.connected_at).num_seconds();
            active_miner = Some(serde_json::json!({
                "wallet": miner.wallet,
                "miner_name": miner.name,
                "ip": miner.ip,
                "pool_name": miner.pool_name,
                "shares_accepted": miner.shares_accepted.load(std::sync::atomic::Ordering::Relaxed),
                "shares_rejected": miner.shares_rejected.load(std::sync::atomic::Ordering::Relaxed),
                "bytes_download": miner.bytes_download.load(std::sync::atomic::Ordering::Relaxed),
                "bytes_upload": miner.bytes_upload.load(std::sync::atomic::Ordering::Relaxed),
                "packets_sent": miner.packets_sent.load(std::sync::atomic::Ordering::Relaxed),
                "packets_received": miner.packets_received.load(std::sync::atomic::Ordering::Relaxed),
                "current_hashrate": MinerInfo::format_hashrate(miner.current_hashrate),
                "average_hashrate": MinerInfo::format_hashrate(miner.average_hashrate),
                "difficulty": miner.difficulty,
                "uptime_seconds": uptime,
                "connected_at": miner.connected_at.to_rfc3339(),
                "last_seen": miner.last_seen.to_rfc3339(),
                "status": "online",
            }));
            break;
        }
    }

    let historical_data = if let Some(ref db) = state.database {
        match db.get_miner_by_wallet(&wallet).await {
            Ok(records) => {
                records.iter().map(|r| {
                    serde_json::json!({
                        "wallet": r.wallet,
                        "miner_name": r.miner_name,
                        "ip": r.ip,
                        "pool_name": r.pool_name,
                        "shares_accepted": r.shares_accepted,
                        "shares_rejected": r.shares_rejected,
                        "bytes_download": r.bytes_download,
                        "bytes_upload": r.bytes_upload,
                        "packets_sent": r.packets_sent,
                        "packets_received": r.packets_received,
                        "current_hashrate": MinerInfo::format_hashrate(r.current_hashrate),
                        "average_hashrate": MinerInfo::format_hashrate(r.average_hashrate),
                        "connected_at": r.connected_at,
                        "last_seen": r.last_seen,
                    })
                }).collect::<Vec<_>>()
            }
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let response = serde_json::json!({
        "wallet": wallet,
        "active_miner": active_miner,
        "historical_data": historical_data,
        "total_miners": historical_data.len(),
    });

    Json(response)
}

#[derive(Deserialize)]
struct NetworkStatsQuery {
    hours: Option<u32>,
}

async fn handle_network_stats(
    Query(params): Query<NetworkStatsQuery>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let hours = params.hours.unwrap_or(24);

    let response = serde_json::json!({
        "hours": hours,
        "data_points": 0,
        "stats": [],
    });

    Json(response)
}

#[derive(Deserialize)]
struct SharesStatsQuery {
    wallet: Option<String>,
    hours: Option<u32>,
}

async fn handle_shares_stats(
    Query(params): Query<SharesStatsQuery>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let hours = params.hours.unwrap_or(24);

    let response = serde_json::json!({
        "wallet": params.wallet,
        "hours": hours,
        "total_shares": 0,
        "accepted_count": 0,
        "rejected_count": 0,
        "acceptance_rate": 0.0,
        "shares": [],
    });

    Json(response)
}

async fn handle_prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let metrics = state.system_metrics.read().await;
    let pools = state.pool_manager.get_all_pools().await;
    let miners = state.miner_manager.get_all_miners().await;

    let mut output = String::new();

    output.push_str("# HELP mining_tunnel_uptime_seconds Uptime in seconds\n");
    output.push_str("# TYPE mining_tunnel_uptime_seconds gauge\n");
    output.push_str(&format!("mining_tunnel_uptime_seconds {}\n\n", metrics.uptime.as_secs()));

    output.push_str("# HELP mining_tunnel_active_miners Number of active miners\n");
    output.push_str("# TYPE mining_tunnel_active_miners gauge\n");
    output.push_str(&format!("mining_tunnel_active_miners {}\n\n", metrics.active_miners));

    output.push_str("# HELP mining_tunnel_cpu_usage_percent CPU usage percentage\n");
    output.push_str("# TYPE mining_tunnel_cpu_usage_percent gauge\n");
    output.push_str(&format!("mining_tunnel_cpu_usage_percent {:.2}\n\n", metrics.cpu_usage));

    output.push_str("# HELP mining_tunnel_cpu_cores Number of CPU cores\n");
    output.push_str("# TYPE mining_tunnel_cpu_cores gauge\n");
    output.push_str(&format!("mining_tunnel_cpu_cores {}\n\n", metrics.cpu_cores));

    output.push_str("# HELP mining_tunnel_ram_bytes RAM usage in bytes\n");
    output.push_str("# TYPE mining_tunnel_ram_bytes gauge\n");
    output.push_str(&format!("mining_tunnel_ram_bytes{{type=\"total\"}} {}\n", metrics.ram_total));
    output.push_str(&format!("mining_tunnel_ram_bytes{{type=\"used\"}} {}\n\n", metrics.ram_used));

    for pool_arc in pools {
        let pool = pool_arc.read().await;
        output.push_str(&format!("mining_tunnel_pool_ping_ms{{pool=\"{}\",type=\"current\"}} {:.2}\n",
            pool.name, pool.current_ping));
        output.push_str(&format!("mining_tunnel_pool_ping_ms{{pool=\"{}\",type=\"average\"}} {:.2}\n\n",
            pool.name, pool.average_ping));

        output.push_str(&format!("mining_tunnel_pool_shares_total{{pool=\"{}\",status=\"accepted\"}} {}\n",
            pool.name, pool.shares_accepted));
        output.push_str(&format!("mining_tunnel_pool_shares_total{{pool=\"{}\",status=\"rejected\"}} {}\n\n",
            pool.name, pool.shares_rejected));
    }

    for miner_arc in miners {
        let miner = miner_arc.read().await;
        if !miner.wallet.is_empty() {
            output.push_str(&format!("mining_tunnel_miner_hashrate{{wallet=\"{}\",miner=\"{}\",type=\"current\"}} {:.2}\n",
                miner.wallet, miner.name, miner.current_hashrate));
            output.push_str(&format!("mining_tunnel_miner_hashrate{{wallet=\"{}\",miner=\"{}\",type=\"average\"}} {:.2}\n",
                miner.wallet, miner.name, miner.average_hashrate));
        }
    }

    output
}

async fn handle_websocket(
    ws: WebSocketUpgrade,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|_socket| async {
        // WebSocket logic here
    })
}

fn get_file_size(path: &str) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
