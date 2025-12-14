use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use anyhow::Result;

pub struct Database {
    data_pool: SqlitePool,
    system_pool: SqlitePool,
}

impl Database {
    pub async fn new(data_path: &str, system_path: &str) -> Result<Self> {
        let data_pool = SqlitePool::connect(data_path).await?;
        let system_pool = SqlitePool::connect(system_path).await?;

        let db = Self { data_pool, system_pool };
        db.create_tables().await?;
        
        Ok(db)
    }

    async fn create_tables(&self) -> Result<()> {
        // Data DB tables
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS miners (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallet TEXT NOT NULL,
                miner_name TEXT,
                ip TEXT,
                pool_name TEXT,
                shares_accepted INTEGER DEFAULT 0,
                shares_rejected INTEGER DEFAULT 0,
                bytes_download INTEGER DEFAULT 0,
                bytes_upload INTEGER DEFAULT 0,
                packets_sent INTEGER DEFAULT 0,
                packets_received INTEGER DEFAULT 0,
                current_hashrate REAL DEFAULT 0,
                average_hashrate REAL DEFAULT 0,
                connected_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(wallet, ip, miner_name)
            )
        "#).execute(&self.data_pool).await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet ON miners(wallet)")
            .execute(&self.data_pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_ip ON miners(ip)")
            .execute(&self.data_pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_last_seen ON miners(last_seen)")
            .execute(&self.data_pool).await?;

        // System DB tables
        sqlx::query("PRAGMA journal_mode=WAL").execute(&self.system_pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL").execute(&self.system_pool).await?;
        
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS shares (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallet TEXT NOT NULL,
                miner_name TEXT,
                ip TEXT,
                pool_name TEXT,
                job_id TEXT,
                accepted INTEGER,
                difficulty REAL DEFAULT 0,
                submitted_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        "#).execute(&self.system_pool).await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_shares_wallet ON shares(wallet)")
            .execute(&self.system_pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_shares_submitted ON shares(submitted_at)")
            .execute(&self.system_pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_shares_pool ON shares(pool_name)")
            .execute(&self.system_pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS network_traffic (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                bytes_download INTEGER DEFAULT 0,
                bytes_upload INTEGER DEFAULT 0,
                packets_sent INTEGER DEFAULT 0,
                packets_received INTEGER DEFAULT 0
            )
        "#).execute(&self.system_pool).await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_traffic_timestamp ON network_traffic(timestamp)")
            .execute(&self.system_pool).await?;

        Ok(())
    }

    pub async fn save_share(&self, share: ShareRecord) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO shares (wallet, miner_name, ip, pool_name, job_id, accepted, difficulty, submitted_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#)
        .bind(&share.wallet)
        .bind(&share.miner_name)
        .bind(&share.ip)
        .bind(&share.pool_name)
        .bind(&share.job_id)
        .bind(if share.accepted { 1 } else { 0 })
        .bind(share.difficulty)
        .bind(share.submitted_at.to_rfc3339())
        .execute(&self.system_pool)
        .await?;
        
        Ok(())
    }

    pub async fn save_miner(&self, miner: &crate::miner::MinerInfo) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO miners (wallet, miner_name, ip, pool_name, shares_accepted, shares_rejected,
                bytes_download, bytes_upload, packets_sent, packets_received,
                current_hashrate, average_hashrate, connected_at, last_seen)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(wallet, ip, miner_name) DO UPDATE SET
                shares_accepted = shares_accepted + excluded.shares_accepted,
                shares_rejected = shares_rejected + excluded.shares_rejected,
                bytes_download = bytes_download + excluded.bytes_download,
                bytes_upload = bytes_upload + excluded.bytes_upload,
                packets_sent = packets_sent + excluded.packets_sent,
                packets_received = packets_received + excluded.packets_received,
                current_hashrate = excluded.current_hashrate,
                average_hashrate = excluded.average_hashrate,
                last_seen = excluded.last_seen,
                pool_name = excluded.pool_name
        "#)
        .bind(&miner.wallet)
        .bind(&miner.name)
        .bind(&miner.ip)
        .bind(&miner.pool_name)
        .bind(miner.shares_accepted.load(std::sync::atomic::Ordering::Relaxed))
        .bind(miner.shares_rejected.load(std::sync::atomic::Ordering::Relaxed))
        .bind(miner.bytes_download.load(std::sync::atomic::Ordering::Relaxed))
        .bind(miner.bytes_upload.load(std::sync::atomic::Ordering::Relaxed))
        .bind(miner.packets_sent.load(std::sync::atomic::Ordering::Relaxed))
        .bind(miner.packets_received.load(std::sync::atomic::Ordering::Relaxed))
        .bind(miner.current_hashrate)
        .bind(miner.average_hashrate)
        .bind(miner.connected_at.to_rfc3339())
        .bind(miner.last_seen.to_rfc3339())
        .execute(&self.data_pool)
        .await?;
        
        Ok(())
    }

    pub async fn get_miner_by_wallet(&self, wallet: &str) -> Result<Vec<MinerRecord>> {
        let rows = sqlx::query(r#"
            SELECT wallet, miner_name, ip, pool_name, shares_accepted, shares_rejected,
                bytes_download, bytes_upload, packets_sent, packets_received,
                current_hashrate, average_hashrate, connected_at, last_seen
            FROM miners WHERE wallet LIKE ?
        "#)
        .bind(format!("{}%", wallet))
        .fetch_all(&self.data_pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(MinerRecord {
                wallet: row.get("wallet"),
                miner_name: row.get("miner_name"),
                ip: row.get("ip"),
                pool_name: row.get("pool_name"),
                shares_accepted: row.get("shares_accepted"),
                shares_rejected: row.get("shares_rejected"),
                bytes_download: row.get("bytes_download"),
                bytes_upload: row.get("bytes_upload"),
                packets_sent: row.get("packets_sent"),
                packets_received: row.get("packets_received"),
                current_hashrate: row.get("current_hashrate"),
                average_hashrate: row.get("average_hashrate"),
                connected_at: row.get("connected_at"),
                last_seen: row.get("last_seen"),
            });
        }

        Ok(results)
    }

    pub async fn cleanup_old_data(&self) -> Result<()> {
        sqlx::query("DELETE FROM shares WHERE submitted_at < datetime('now', '-365 days')")
            .execute(&self.system_pool).await?;
        
        sqlx::query("DELETE FROM network_traffic WHERE timestamp < datetime('now', '-180 days')")
            .execute(&self.system_pool).await?;

        sqlx::query("VACUUM").execute(&self.system_pool).await?;
        sqlx::query("VACUUM").execute(&self.data_pool).await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ShareRecord {
    pub wallet: String,
    pub miner_name: String,
    pub ip: String,
    pub pool_name: String,
    pub job_id: String,
    pub accepted: bool,
    pub difficulty: f64,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MinerRecord {
    pub wallet: String,
    pub miner_name: String,
    pub ip: String,
    pub pool_name: String,
    pub shares_accepted: i64,
    pub shares_rejected: i64,
    pub bytes_download: i64,
    pub bytes_upload: i64,
    pub packets_sent: i64,
    pub packets_received: i64,
    pub current_hashrate: f64,
    pub average_hashrate: f64,
    pub connected_at: String,
    pub last_seen: String,
}