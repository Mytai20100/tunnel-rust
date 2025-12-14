use dashmap::DashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use crate::config::{Config, PoolConfig};

#[derive(Debug, Clone)]
pub struct PoolMetrics {
    pub name: String,
    pub current_ping: f64,
    pub average_ping: f64,
    pub ping_samples: Vec<f64>,
    pub avg_accept_time: f64,
    pub accept_times: Vec<f64>,
    pub shares_accepted: i64,
    pub shares_rejected: i64,
    pub last_ping_time: DateTime<Utc>,
}

impl PoolMetrics {
    pub fn new(name: String) -> Self {
        Self {
            name,
            current_ping: 0.0,
            average_ping: 0.0,
            ping_samples: Vec::new(),
            avg_accept_time: 0.0,
            accept_times: Vec::new(),
            shares_accepted: 0,
            shares_rejected: 0,
            last_ping_time: Utc::now(),
        }
    }

    pub fn add_ping_sample(&mut self, ping: f64) {
        self.current_ping = ping;
        self.ping_samples.push(ping);
        if self.ping_samples.len() > 100 {
            self.ping_samples.remove(0);
        }
        self.average_ping = self.ping_samples.iter().sum::<f64>() / self.ping_samples.len() as f64;
        self.last_ping_time = Utc::now();
    }

    pub fn add_accept_time(&mut self, time: f64) {
        self.accept_times.push(time);
        if self.accept_times.len() > 100 {
            self.accept_times.remove(0);
        }
        self.avg_accept_time = self.accept_times.iter().sum::<f64>() / self.accept_times.len() as f64;
    }
}

pub struct PoolManager {
    pools: Arc<DashMap<String, Arc<tokio::sync::RwLock<PoolMetrics>>>>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(DashMap::new()),
        }
    }

    pub fn get_or_create(&self, name: &str) -> Arc<tokio::sync::RwLock<PoolMetrics>> {
        self.pools.entry(name.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::RwLock::new(PoolMetrics::new(name.to_string()))))
            .clone()
    }

    pub async fn get_all_pools(&self) -> Vec<Arc<tokio::sync::RwLock<PoolMetrics>>> {
        self.pools.iter().map(|entry| Arc::clone(entry.value())).collect()
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn monitor_pool_pings(manager: Arc<PoolManager>, config: Config) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

    loop {
        interval.tick().await;

        for (pool_name, pool_config) in &config.pools {
            let mgr = Arc::clone(&manager);
            let name = pool_name.clone();
            let cfg = pool_config.clone();

            tokio::spawn(async move {
                measure_pool_ping(mgr, &name, &cfg).await;
            });
        }
    }
}

async fn measure_pool_ping(manager: Arc<PoolManager>, name: &str, config: &PoolConfig) {
    let start = std::time::Instant::now();
    let addr = format!("{}:{}", config.host, config.port);

    if tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr)
    ).await.is_ok() {
        let ping_ms = start.elapsed().as_secs_f64() * 1000.0;
        let metrics = manager.get_or_create(name);
        metrics.write().await.add_ping_sample(ping_ms);
    }
}
