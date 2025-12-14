use sysinfo::{System, SystemExt, DiskExt, CpuExt};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::miner::MinerManager;

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    pub ram_total: u64,
    pub ram_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub os: String,
    pub public_ip: String,
    pub uptime: std::time::Duration,
    pub active_miners: usize,
}

impl SystemMetrics {
    pub async fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_model = sys.global_cpu_info().brand().to_string();
        let cpu_cores = sys.cpus().len();

        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();

        let disk_total: u64 = sys.disks().iter().map(|d| d.total_space()).sum();
        let disk_used: u64 = sys.disks().iter().map(|d| d.total_space() - d.available_space()).sum();

        let os = format!("{} {}", 
            sys.name().unwrap_or_else(|| "Unknown".to_string()),
            sys.os_version().unwrap_or_else(|| "Unknown".to_string())
        );

        let public_ip = Self::get_public_ip().await.unwrap_or_else(|| "Unknown".to_string());

        Self {
            cpu_model,
            cpu_cores,
            cpu_usage: 0.0,
            ram_total,
            ram_used,
            disk_total,
            disk_used,
            os,
            public_ip,
            uptime: std::time::Duration::from_secs(0),
            active_miners: 0,
        }
    }

    async fn get_public_ip() -> Option<String> {
        match reqwest::get("https://api.ipify.org?format=text").await {
            Ok(resp) => resp.text().await.ok(),
            Err(_) => None,
        }
    }

    pub fn update(&mut self, sys: &System, active_miners: usize, start_time: std::time::Instant) {
        self.cpu_usage = sys.global_cpu_info().cpu_usage();
        self.ram_used = sys.used_memory();

        self.disk_used = sys.disks().iter()
            .map(|d| d.total_space() - d.available_space())
            .sum();

        self.active_miners = active_miners;
        self.uptime = start_time.elapsed();
    }
}

pub async fn update_system_metrics(
    metrics: Arc<RwLock<SystemMetrics>>,
    miner_manager: Arc<MinerManager>,
) {
    let mut sys = System::new_all();
    let start_time = std::time::Instant::now();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        interval.tick().await;

        sys.refresh_all();

        let active_miners = miner_manager.active_count();

        let mut m = metrics.write().await;
        m.update(&sys, active_miners, start_time);
    }
}