use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;

#[derive(Debug)]
pub struct MinerInfo {
    pub wallet: String,
    pub name: String,
    pub ip: String,
    pub port: String,
    pub pool_name: String,
    pub job_id: String,
    pub shares_accepted: AtomicI64,
    pub shares_rejected: AtomicI64,
    pub last_seen: DateTime<Utc>,
    pub connected_at: DateTime<Utc>,
    pub bytes_download: AtomicI64,
    pub bytes_upload: AtomicI64,
    pub packets_sent: AtomicI64,
    pub packets_received: AtomicI64,
    pub last_share_time: DateTime<Utc>,
    pub share_times: Vec<DateTime<Utc>>,
    pub current_hashrate: f64,
    pub average_hashrate: f64,
    pub difficulty: f64,
}

impl MinerInfo {
    pub fn new(ip: String, port: String, pool_name: String) -> Self {
        Self {
            wallet: String::new(),
            name: "Unknown".to_string(),
            ip,
            port,
            pool_name,
            job_id: String::new(),
            shares_accepted: AtomicI64::new(0),
            shares_rejected: AtomicI64::new(0),
            last_seen: Utc::now(),
            connected_at: Utc::now(),
            bytes_download: AtomicI64::new(0),
            bytes_upload: AtomicI64::new(0),
            packets_sent: AtomicI64::new(0),
            packets_received: AtomicI64::new(0),
            last_share_time: Utc::now(),
            share_times: Vec::new(),
            current_hashrate: 0.0,
            average_hashrate: 0.0,
            difficulty: 1.0,
        }
    }

    pub fn calculate_hashrate(&mut self) {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::minutes(10);
        
        self.share_times.retain(|&t| t > cutoff);
        
        if self.share_times.len() < 2 {
            self.current_hashrate = 0.0;
            return;
        }

        let total_time = (self.share_times.last().unwrap().timestamp() 
            - self.share_times.first().unwrap().timestamp()) as f64;
        
        if total_time > 0.0 {
            let shares_per_second = self.share_times.len() as f64 / total_time;
            self.current_hashrate = shares_per_second * self.difficulty;
        }

        if self.average_hashrate == 0.0 {
            self.average_hashrate = self.current_hashrate;
        } else {
            self.average_hashrate = (self.average_hashrate * 0.9) + (self.current_hashrate * 0.1);
        }
    }

    pub fn format_hashrate(hashrate: f64) -> String {
        if hashrate == 0.0 {
            return "0 H/s".to_string();
        }

        let units = ["H/s", "KH/s", "MH/s", "GH/s", "TH/s", "PH/s"];
        let mut unit_index = 0;
        let mut value = hashrate;

        while value >= 1000.0 && unit_index < units.len() - 1 {
            value /= 1000.0;
            unit_index += 1;
        }

        if value >= 100.0 {
            format!("{:.0} {}", value, units[unit_index])
        } else if value >= 10.0 {
            format!("{:.1} {}", value, units[unit_index])
        } else {
            format!("{:.2} {}", value, units[unit_index])
        }
    }
}

pub struct MinerManager {
    miners: Arc<DashMap<String, Arc<tokio::sync::RwLock<MinerInfo>>>>,
}

impl MinerManager {
    pub fn new() -> Self {
        Self {
            miners: Arc::new(DashMap::new()),
        }
    }

    pub fn add_miner(&self, key: String, miner: MinerInfo) {
        self.miners.insert(key, Arc::new(tokio::sync::RwLock::new(miner)));
    }

    pub fn get_miner(&self, key: &str) -> Option<Arc<tokio::sync::RwLock<MinerInfo>>> {
        self.miners.get(key).map(|m| Arc::clone(m.value()))
    }

    pub fn remove_miner(&self, key: &str) -> Option<Arc<tokio::sync::RwLock<MinerInfo>>> {
        self.miners.remove(key).map(|(_, m)| m)
    }

    pub fn active_count(&self) -> usize {
        self.miners.len()
    }

    pub async fn get_all_miners(&self) -> Vec<Arc<tokio::sync::RwLock<MinerInfo>>> {
        self.miners.iter().map(|entry| Arc::clone(entry.value())).collect()
    }
}