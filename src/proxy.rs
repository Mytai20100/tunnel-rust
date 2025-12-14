use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use anyhow::Result;
use colored::Colorize;
use crate::{config::*, miner::*, pool::*, database::*};

pub async fn start_tunnel(
    name: &str,
    tunnel_config: TunnelConfig,
    pool_config: PoolConfig,
    miner_manager: Arc<MinerManager>,
    pool_manager: Arc<PoolManager>,
    database: Option<Arc<Database>>,
    _tls_enabled: bool,
    _cert_file: &str,
    _key_file: &str,
    nodebug: bool,
) -> Result<()> {
    let addr = format!("{}:{}", tunnel_config.ip, tunnel_config.port);
    let listener = TcpListener::bind(&addr).await?;

    if !nodebug {
        println!("{}", format!("Tunnel {} listening on {} -> {}:{} ({})",
            name, addr, pool_config.host, pool_config.port, pool_config.name).bright_blue());
    }

    loop {
        let (client_conn, client_addr) = listener.accept().await?;
        
        let miner_mgr = Arc::clone(&miner_manager);
        let pool_mgr = Arc::clone(&pool_manager);
        let db = database.clone();
        let pool_cfg = pool_config.clone();
        let tunnel_name = name.to_string();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                client_conn,
                client_addr.to_string(),
                &tunnel_name,
                pool_cfg,
                miner_mgr,
                pool_mgr,
                db,
                nodebug,
            ).await {
                if !nodebug {
                    eprintln!("{}", format!("Connection error: {}", e).red());
                }
            }
        });
    }
}

async fn handle_connection(
    client_conn: TcpStream,
    client_addr: String,
    _tunnel_name: &str,
    pool_config: PoolConfig,
    miner_manager: Arc<MinerManager>,
    pool_manager: Arc<PoolManager>,
    database: Option<Arc<Database>>,
    nodebug: bool,
) -> Result<()> {
    let (client_ip, client_port) = client_addr.split_once(':').unwrap_or(("unknown", "0"));
    
    if !nodebug {
        println!("{}", format!("New connection from {}", client_addr).bright_cyan());
    }

    let pool_addr = format!("{}:{}", pool_config.host, pool_config.port);
    let pool_conn = TcpStream::connect(&pool_addr).await?;

    let miner_key = format!("{}:{}", client_ip, client_port);
    let miner = MinerInfo::new(client_ip.to_string(), client_port.to_string(), pool_config.name.clone());
    miner_manager.add_miner(miner_key.clone(), miner);

    let (client_reader, client_writer) = client_conn.into_split();
    let (pool_reader, pool_writer) = pool_conn.into_split();

    let mut client_buf = BufReader::new(client_reader);
    let mut pool_buf = BufReader::new(pool_reader);

    let miner_mgr_c2p = Arc::clone(&miner_manager);
    let miner_key_c2p = miner_key.clone();
    let pool_cfg_c2p = pool_config.clone();
    let mut pool_writer_c2p = pool_writer;

    // Client to Pool
    let c2p = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            match client_buf.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if pool_writer_c2p.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                    
                    if let Some(miner) = miner_mgr_c2p.get_miner(&miner_key_c2p) {
                        let m = miner.write().await;
                        m.bytes_upload.fetch_add(n as i64, std::sync::atomic::Ordering::Relaxed);
                        m.packets_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }

                    parse_client_message(&line, &miner_key_c2p, &miner_mgr_c2p, &pool_cfg_c2p, nodebug).await;
                }
            }
        }
    });

    let miner_mgr_p2c = Arc::clone(&miner_manager);
    let miner_key_p2c = miner_key.clone();
    let pool_mgr_p2c = Arc::clone(&pool_manager);
    let pool_cfg_p2c = pool_config.clone();
    let db_p2c = database.clone();
    let mut client_writer_p2c = client_writer;

    // Pool to Client
    let p2c = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            match pool_buf.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if client_writer_p2c.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                    
                    if let Some(miner) = miner_mgr_p2c.get_miner(&miner_key_p2c) {
                        let m = miner.write().await;
                        m.bytes_download.fetch_add(n as i64, std::sync::atomic::Ordering::Relaxed);
                        m.packets_received.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }

                    parse_pool_message(&line, &miner_key_p2c, &miner_mgr_p2c, &pool_mgr_p2c, 
                        &pool_cfg_p2c, &db_p2c, nodebug).await;
                }
            }
        }
    });

    tokio::select! {
        _ = c2p => {},
        _ = p2c => {},
    }

    if let Some(miner_arc) = miner_manager.remove_miner(&miner_key) {
        if let Some(db) = database {
            let miner = miner_arc.read().await;
            let _ = db.save_miner(&*miner).await;
        }
    }

    if !nodebug {
        println!("{}", format!("Connection closed for {}", client_addr).yellow());
    }

    Ok(())
}

async fn parse_client_message(
    message: &str,
    miner_key: &str,
    miner_manager: &Arc<MinerManager>,
    pool_config: &PoolConfig,
    nodebug: bool,
) {
    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(message) {
        if let Some(miner_arc) = miner_manager.get_miner(miner_key) {
            let mut miner = miner_arc.write().await;
            
            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                match method {
                    "mining.authorize" => {
                        if let Some(params) = msg.get("params").and_then(|p| p.as_array()) {
                            if let Some(username) = params.get(0).and_then(|u| u.as_str()) {
                                let parts: Vec<&str> = username.split('.').collect();
                                miner.wallet = parts[0].to_string();
                                miner.name = username.to_string();
                                
                                if !nodebug {
                                    println!("{}", format!("Miner {} ({}:{}) authorized on {} -> {}",
                                        username, miner.ip, miner.port, pool_config.name, pool_config.name).green());
                                }
                            }
                        }
                    }
                    "mining.submit" => {
                        if let Some(params) = msg.get("params").and_then(|p| p.as_array()) {
                            if let Some(job_id) = params.get(1).and_then(|j| j.as_str()) {
                                miner.job_id = job_id.to_string();
                            }
                        }
                        miner.last_share_time = chrono::Utc::now();
                        miner.share_times.push(chrono::Utc::now());
                        
                        if !nodebug {
                            println!("{}", format!("Share submitted: {} ({}:{}) job={} pool={}",
                                miner.name, miner.ip, miner.port, miner.job_id, pool_config.name).bright_purple());
                        }
                    }
                    _ => {}
                }
            }
            
            miner.last_seen = chrono::Utc::now();
        }
    }
}

async fn parse_pool_message(
    message: &str,
    miner_key: &str,
    miner_manager: &Arc<MinerManager>,
    pool_manager: &Arc<PoolManager>,
    pool_config: &PoolConfig,
    database: &Option<Arc<Database>>,
    nodebug: bool,
) {
    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(message) {
        if let Some(miner_arc) = miner_manager.get_miner(miner_key) {
            let mut miner = miner_arc.write().await;
            
            if let Some(error) = msg.get("error") {
                if !error.is_null() {
                    if !nodebug {
                        println!("{}", format!("Error from pool {}: {:?}", pool_config.name, error).red());
                    }
                }
            }

            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                match method {
                    "mining.notify" => {
                        if let Some(params) = msg.get("params").and_then(|p| p.as_array()) {
                            if let Some(job_id) = params.get(0).and_then(|j| j.as_str()) {
                                miner.job_id = job_id.to_string();
                                if !nodebug {
                                    println!("{}", format!("New job {} for miner {} from pool {}",
                                        job_id, miner.name, pool_config.name).bright_blue());
                                }
                            }
                        }
                    }
                    "mining.set_difficulty" => {
                        if let Some(params) = msg.get("params").and_then(|p| p.as_array()) {
                            if let Some(diff) = params.get(0).and_then(|d| d.as_f64()) {
                                miner.difficulty = diff;
                                if !nodebug {
                                    println!("{}", format!("Difficulty set to {:.2} for miner {}", diff, miner.name).bright_blue());
}
}
}
}
_ => {}
}
}
        if msg.get("id").is_some() {
            if let Some(result) = msg.get("result") {
                if let Some(accepted) = result.as_bool() {
                    let submit_time = (chrono::Utc::now() - miner.last_share_time).num_milliseconds() as f64;

                    if accepted {
                        miner.shares_accepted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        miner.calculate_hashrate();

                        let pool_metrics = pool_manager.get_or_create(&pool_config.name);
                        {
                            let mut pm = pool_metrics.write().await;
                            pm.shares_accepted += 1;
                            pm.add_accept_time(submit_time);
                        }

                        if let Some(db) = database {
                            let db_clone = Arc::clone(db);
                            let share = ShareRecord {
                                wallet: miner.wallet.clone(),
                                miner_name: miner.name.clone(),
                                ip: miner.ip.clone(),
                                pool_name: pool_config.name.clone(),
                                job_id: miner.job_id.clone(),
                                accepted: true,
                                difficulty: miner.difficulty,
                                submitted_at: chrono::Utc::now(),
                            };
                            tokio::spawn(async move {
                                let _ = db_clone.save_share(share).await;
                            });
                        }

                        if !nodebug {
                            println!("{}", format!("✓ ACCEPTED: {} ({}:{}) pool={} ({:.0}ms) [curr={} avg={}]",
                                miner.name, miner.ip, miner.port, pool_config.name, submit_time,
                                MinerInfo::format_hashrate(miner.current_hashrate),
                                MinerInfo::format_hashrate(miner.average_hashrate)).green());
                        }
                    } else {
                        miner.shares_rejected.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        let pool_metrics = pool_manager.get_or_create(&pool_config.name);
                        {
                            let mut pm = pool_metrics.write().await;
                            pm.shares_rejected += 1;
                        }

                        if let Some(db) = database {
                            let db_clone = Arc::clone(db);
                            let share = ShareRecord {
                                wallet: miner.wallet.clone(),
                                miner_name: miner.name.clone(),
                                ip: miner.ip.clone(),
                                pool_name: pool_config.name.clone(),
                                job_id: miner.job_id.clone(),
                                accepted: false,
                                difficulty: miner.difficulty,
                                submitted_at: chrono::Utc::now(),
                            };
                            tokio::spawn(async move {
                                let _ = db_clone.save_share(share).await;
                            });
                        }

                        if !nodebug {
                            println!("{}", format!("✗ REJECTED: {} ({}:{}) pool={}",
                                miner.name, miner.ip, miner.port, pool_config.name).red());
                        }
                    }
                }
            }
        }

        miner.last_seen = chrono::Utc::now();
    }
}
}