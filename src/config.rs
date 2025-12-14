use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::fs;
use colored::Colorize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub pools: HashMap<String, PoolConfig>,
    pub tunnels: HashMap<String, TunnelConfig>,
    pub api_port: u16,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub host: String,
    pub port: u16,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub ip: String,
    pub port: u16,
    pub pool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
}

impl Config {
    pub async fn load_or_create(path: &str) -> anyhow::Result<Self> {
        if tokio::fs::metadata(path).await.is_ok() {
            let content = fs::read_to_string(path).await?;
            Ok(serde_yaml::from_str(&content)?)
        } else {
            let config = Self::default();
            let yaml = serde_yaml::to_string(&config)?;
            fs::write(path, yaml).await?;
            println!("{}", "Created default config.yml".bright_yellow());
            Ok(config)
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut pools = HashMap::new();
        pools.insert("pool1".to_string(), PoolConfig {
            host: "pool.example.com".to_string(),
            port: 4444,
            name: "Example Pool".to_string(),
        });

        let mut tunnels = HashMap::new();
        tunnels.insert("tunnel1".to_string(), TunnelConfig {
            ip: "0.0.0.0".to_string(),
            port: 3333,
            pool: "pool1".to_string(),
        });

        Self {
            pools,
            tunnels,
            api_port: 8080,
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 3306,
                user: "root".to_string(),
                password: "password".to_string(),
                dbname: "mining_tunnel".to_string(),
            },
        }
    }
}
