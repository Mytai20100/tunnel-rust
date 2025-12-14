# tunnel-rust

<div align="center">

![Version](https://img.shields.io/badge/version-3.3-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)
![License](https://img.shields.io/badge/license-MIT-green.svg)
![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)
![Stars](https://img.shields.io/github/stars/Mytai20100/tunnel-rust?style=social)
![Forks](https://img.shields.io/github/forks/Mytai20100/tunnel-rust?style=social)
![Issues](https://img.shields.io/github/issues/Mytai20100/tunnel-rust)
![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)

**A fast and reliable mining tunnel built with Rust**

[Features](#features) • [Installation](#installation) • [Usage](#usage) • [API](#api) • [Configuration](#configuration)

</div>

---

## Features

- **High Performance**: Built with Rust for maximum speed and safety
- **Real-time Monitoring**: Live hashrate, shares, and network stats
- **TLS Support**: Secure connections with TLS encryption
- **SQLite Database**: Fast data storage with automatic cleanup
- **REST API**: Full API for integration
- **Prometheus Metrics**: Built-in metrics for monitoring
- **WebSocket Logs**: Real-time log streaming
- **Multi-Pool Support**: Connect to multiple mining pools

---

## Requirements

Before you start, make sure you have:

- **Rust** (latest stable version)
- **Cargo** (comes with Rust)
- **Git**

---

## 🚀 Installation

### Step 1: Install Rust

**Linux/macOS:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Windows:**
Download and install from [rustup.rs](https://rustup.rs/)

### Step 2: Verify Installation
```bash
rustc --version
cargo --version
```

### Step 3: Clone the Repository
```bash
git clone https://github.com/Mytai20100/tunnel-rust.git
cd tunnel-rust
```

---

## Building from Source

### Using the Build Script (Recommended)

**Linux/macOS:**
```bash
chmod +x build.sh
./build.sh
```

The build script will:
1. Install Rust (if needed)
2. Check Rust version
3. Build the release version

### Manual Build

```bash
# Development build (faster, with debug info)
cargo build

# Release build (optimized, slower to build)
cargo build --release
```

The compiled binary will be in:
- Debug: `target/debug/tunnel-rust`
- Release: `target/release/tunnel-rust`

---

## Configuration

Create a `config.yml` file in the project root:

```yaml
pools:
  pool1:
    host: "pool.example.com"
    port: 4444
    name: "Example Pool"
  
  pool2:
    host: "another-pool.com"
    port: 3333
    name: "Another Pool"

tunnels:
  tunnel1:
    ip: "0.0.0.0"
    port: 3333
    pool: "pool1"
  
  tunnel2:
    ip: "0.0.0.0"
    port: 3334
    pool: "pool2"

api_port: 8080

database:
  path: "tunnel.db"
  max_connections: 10
```

---

## Usage

### Basic Commands

```bash
# Run with default settings
./target/release/tunnel-rust

# Show help
./target/release/tunnel-rust --help

# Show version
./target/release/tunnel-rust --version
```

### Command Options

| Option | Description |
|--------|-------------|
| `--nodata` | Disable database logging |
| `--noapi` | Disable API server |
| `--nodebug` | Minimal output (single line status) |
| `--tls` | Enable TLS encryption |
| `--tlscert` | TLS certificate file (default: cert.pem) |
| `--tlskey` | TLS key file (default: key.pem) |

### Examples

```bash
# Run without database
./target/release/tunnel-rust --nodata

# Run with minimal output
./target/release/tunnel-rust --nodebug

# Run with TLS
./target/release/tunnel-rust --tls --tlscert=cert.pem --tlskey=key.pem

# Run without database and API
./target/release/tunnel-rust --nodata --noapi
```

---

## TLS Setup (Optional)

### Generate Self-Signed Certificate

```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
```

### Run with TLS

```bash
./target/release/tunnel-rust --tls --tlscert=cert.pem --tlskey=key.pem
```

---

## API Endpoints

### Get System Metrics
```bash
GET http://localhost:8080/api/metrics
```

### Get Miner Info
```bash
GET http://localhost:8080/api/i/{wallet_address}
```

### Get Network Stats
```bash
GET http://localhost:8080/api/network/stats?hours=24
```

### Get Shares Stats
```bash
GET http://localhost:8080/api/shares/stats?wallet={address}&hours=24
```

### Prometheus Metrics
```bash
GET http://localhost:8080/metrics
```

### WebSocket Logs
```bash
WS ws://localhost:8080/api/logs/stream
```

---

## Example API Response

**GET /api/metrics:**
```json
{
  "system": {
    "cpu_model": "Intel Core i7",
    "cpu_cores": 8,
    "cpu_usage": "25.50%",
    "ram_total": 17179869184,
    "ram_used": 8589934592,
    "uptime": 3600
  },
  "network": {
    "download_gb": 1.25,
    "upload_gb": 0.85,
    "packets_sent": 150000,
    "packets_received": 145000
  },
  "miners": {
    "active": 5,
    "list": [...]
  }
}
```

---

## Docker Support

### Dockerfile
```dockerfile
FROM rust:latest AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /root/
COPY --from=builder /app/target/release/tunnel-rust .
COPY config.yml .

EXPOSE 3333 8080
CMD ["./tunnel-rust"]
```

### Build & Run
```bash
docker build -t tunnel-rust:latest .
docker run -d -p 3333:3333 -p 8080:8080 --name tunnel-rust tunnel-rust:latest
```

---

## Systemd Service (Linux)

Create `/etc/systemd/system/tunnel-rust.service`:

```ini
[Unit]
Description=Rust Mining Tunnel
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/tunnel-rust
ExecStart=/root/tunnel-rust/target/release/tunnel-rust
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### Enable & Start
```bash
sudo systemctl daemon-reload
sudo systemctl enable tunnel-rust
sudo systemctl start tunnel-rust
sudo systemctl status tunnel-rust
```

---

## Development

### Run Tests
```bash
cargo test
```

### Check Code
```bash
cargo check
```

### Format Code
```bash
cargo fmt
```

### Run Linter
```bash
cargo clippy
```

---

## Logs & Debugging

### View Logs
```bash
# Application logs
tail -f /var/log/tunnel-rust.log

# Systemd logs
journalctl -u tunnel-rust -f
```

---

## Contributing

Contributions are welcome! Here's how:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/awesome-feature`)
3. Commit your changes (`git commit -m 'Add awesome feature'`)
4. Push to the branch (`git push origin feature/awesome-feature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- WebSocket support with [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite)
- Database with [rusqlite](https://github.com/rusqlite/rusqlite)
- System metrics with [sysinfo](https://github.com/GuillaumeGomez/sysinfo)

---

## Support

- **Issues**: [GitHub Issues](https://github.com/Mytai20100/tunnel-rust/issues)
- **Discussions**: [GitHub Discussions](https://github.com/Mytai20100/tunnel-rust/discussions)

---

<div align="center">

**Made with ❤️ by [mytai](https://github.com/Mytai20100)**

[Star this repo](https://github.com/Mytai20100/tunnel-rust) • [🐛Report Bug](https://github.com/Mytai20100/tunnel-rust/issues) • [Request Feature](https://github.com/Mytai20100/tunnel-rust/issues)

![GitHub last commit](https://img.shields.io/github/last-commit/Mytai20100/tunnel-rust)
![GitHub commit activity](https://img.shields.io/github/commit-activity/m/Mytai20100/tunnel-rust)
![GitHub contributors](https://img.shields.io/github/contributors/Mytai20100/tunnel-rust)

</div>
