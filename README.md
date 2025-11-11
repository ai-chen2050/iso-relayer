# ISO Relayer

> A high-performance and scalable Nostr protocol relay service

## Project Overview

**ISO Relayer** is a high-performance Nostr protocol relay service implemented in Rust. It is designed to handle large-scale event streams with efficient event deduplication, routing, and forwarding capabilities.

### Core Features

- 🚀 **High-Performance Event Processing**: Using async Rust and Tokio runtime, supporting high-throughput event processing
- 🔄 **Smart Deduplication**: Three-layer architecture with Bloom filter + LRU cache + RocksDB for efficient deduplication
- 📊 **Complete Monitoring Metrics**: Integrated Prometheus metrics system for real-time system monitoring
- 🔌 **Multiple Output Methods**: Support for WebSocket, TCP, and HTTP REST forwarding
- 💾 **Persistent Storage**: RocksDB for efficient local storage and fast queries
- 🌐 **Relay Pool Management**: Support for connecting to multiple Nostr relays with automatic health checks
- ⚙️ **Flexible Configuration**: TOML configuration files and environment variables for customizable deployment

## System Architecture

```mermaid
graph TB
    subgraph ExternalRelays [External Nostr Relay Nets]
        direction LR
        ER1[Relay 1]
        ER2[Relay 2]
        ER3[Relay 3]
        ER4[Relay..10,000]
    end
    
    subgraph RelayGateway [Relay Gateway Core]
        RP[Relay Connection Pool]
        DE[Deduplication Engine]
        ES[Event Stream Processor]
        ST[Storage Layer]
        API[Output Interface]
    end
    
    subgraph Downstream [PoCW Engine]
        DS1[IFC: Intersubjective Fluxional Consensus]
        DS2[ISO: Intersubjective Semantic Organism]
    end
    
    ER1 --> RP
    ER2 --> RP
    ER3 --> RP
    ER4 --> RP
    
    RP --> DE
    DE --> ES
    ES --> API
    ST --> DE
    ST --> ES
    
    API --> DS1
    API --> DS2
    
    RelayGateway -.-> Prometheus/RESTAPI
```

## Project Structure

```
iso-relayer/
├── src/
│   ├── main.rs                 # Main program entry point
│   ├── config/                 # Configuration management module
│   │   └── mod.rs              # Configuration struct definitions
│   ├── api/                    # API interface layer
│   │   ├── rest_api.rs         # REST API routing
│   │   ├── websocket.rs        # WebSocket handling
│   │   ├── metrics.rs          # Prometheus metrics
│   │   └── mod.rs              # API module exports
│   ├── core/                   # Core business logic
│   │   ├── relay_pool.rs       # Relay connection pool management
│   │   ├── event_router.rs     # Event routing engine
│   │   ├── dedupe_engine.rs    # Deduplication engine
│   │   ├── downstream.rs       # Downstream forwarder
│   │   └── mod.rs              # Core module exports
│   └── storage/                # Storage layer
│       ├── rocksdb_store.rs    # RocksDB storage implementation
│       ├── bloom_filter.rs     # Bloom filter
│       ├── memory_cache.rs     # Memory cache
│       └── mod.rs              # Storage module exports
├── Cargo.toml                  # Rust dependency configuration
├── Cargo.lock                  # Dependency version lock
├── Dockerfile                  # Docker container configuration
├── config.template.toml        # Configuration file template
├── Makefile                    # Build and deployment scripts
└── README.md                   # Project documentation
```

## Key Module Description

### 1. Relay Pool (src/core/relay_pool.rs)

- **Function**: Manage connections to multiple Nostr relays
- **Features**:
  - Connection pool management with configurable maximum connections
  - Periodic health checks to detect relay availability
  - Event receipt and distribution from relays
  - Automatic reconnection mechanism

### 2. Deduplication Engine (src/core/dedupe_engine.rs)

- **Function**: Efficient event deduplication
- **Three-Layer Architecture**:
  - **Hotset**: Most recently active events stored in memory
  - **Bloom Filter**: Fast event existence detection
  - **LRU Cache**: Recently used event caching
  - **RocksDB**: Persistent storage for all historical events

### 3. Event Router (src/core/event_router.rs)

- **Function**: Event stream routing and batch processing
- **Features**:
  - Batch process events for improved throughput
  - Configurable batch size and latency
  - Event deduplication and forwarding

### 4. Downstream Forwarder (src/core/downstream.rs)

- **Function**: Forward processed events downstream
- **Supported Transport Methods**:
  - WebSocket (default)
  - TCP direct connection
  - HTTP REST interface

### 5. Metrics (src/api/metrics.rs)

- **Monitoring Metrics**:
  - `events_processed_total`: Total events processed
  - `duplicates_filtered_total`: Duplicate events filtered
  - `processing_latency_seconds`: Event processing latency
  - `memory_usage_bytes`: Memory usage
  - `active_connections`: Active connections count
  - `events_in_queue`: Events waiting in queue

## Quick Start

### Prerequisites

- Rust 1.89.0+ (latest stable version recommended)
- Cargo (Rust package manager)
- macOS or Linux (Windows requires WSL)

### Installation

1. **Clone the project**

   ```bash
   cd /Users/blake/work/nagara/code/Mining/iso-relayer
   ```

2. **Create configuration file**

   ```bash
   cp config.template.toml config.toml
   # Edit config.toml to customize as needed
   ```

3. **Compile and run**
   ```bash
   make build      # Build the project
   make run        # Run the project
   # Or
   make dev        # Development mode (debug build)
   ```

### Using Makefile

The project includes a Makefile with convenient build commands:

```bash
make help           # Show all available commands
make build          # Build release version (optimized)
make dev            # Build development version (debug)
make run            # Run release version
make debug          # Run development version
make test           # Run unit tests
make bench          # Run benchmark tests
make clean          # Clean build artifacts
make fmt            # Format code
make lint           # Code check
make release        # Build release and show binary path
make docker-build   # Build Docker image
make docker-run     # Run Docker container
make docker-push    # Push Docker image
```

### Direct Execution

```bash
# Use default relays (example relays)
cargo run --release

# Use configuration file
cargo run --release -- --config config.toml

# Set log level
RUST_LOG=iso_relayer=debug cargo run --release
```

## API Endpoints

Please refer to [docs/API.md](docs/API.md) for detailed API endpoint documentation.

## Configuration File

### Configuration Template (config.template.toml)

```toml
[relay]
# Relay connection configuration
health_check_interval = 30      # Health check interval (seconds)
max_connections = 10000         # Maximum connections
bootstrap_relays = [            # Bootstrap relay list
  "wss://relay.damus.io",
  "wss://nos.lol",
]

[deduplication]
# Deduplication engine configuration
rocksdb_path = "./data/rocksdb" # RocksDB data path
hotset_size = 10000             # Hotset size
bloom_capacity = 1000000        # Bloom filter capacity
lru_size = 50000                # LRU cache size

[output]
# Output configuration
websocket_enabled = true        # Enable WebSocket
websocket_port = 3000           # WebSocket port
batch_size = 100                # Batch processing size
max_latency_ms = 100            # Maximum latency (milliseconds)
downstream_tcp = []             # TCP downstream endpoints
downstream_rest = []            # REST downstream endpoints

[monitoring]
# Monitoring configuration
log_level = "info"              # Log level (trace/debug/info/warn/error)
prometheus_port = 9090          # Prometheus port
```

## Performance Features

### Deduplication Efficiency

- **Bloom Filter**: O(1) time complexity for fast event existence checks
- **LRU Cache**: Hot event caching reduces database queries
- **RocksDB**: Efficient LSM tree structure optimized for batch writes and range queries

### Throughput

- Support processing millions of events per second
- Asynchronous event processing utilizes multiple CPU cores
- Batch processing mechanism reduces system overhead

### Memory Management

- Configurable cache sizes prevent memory overflow
- Regular memory usage metric updates
- Memory usage monitoring and alerting support

## Operations and Deployment

### Docker Deployment

```bash
# Build Docker image
make docker-build

# Run Docker container
make docker-run

# View container logs
docker logs iso-relayer

# Stop container
docker stop iso-relayer
```

### Production Environment Recommendations

1. **Configuration Optimization**

   - Adjust `hotset_size`, `lru_size`, `max_connections` based on machine resources
   - Set reasonable batch size and latency
   - Use RocksDB performance tuning parameters

2. **Monitoring and Alerting**

   - Integrate Prometheus and Grafana for visualization
   - Set alert rules for key metrics
   - Monitor memory usage, event processing latency, etc.

3. **High Availability Deployment**

   - Use load balancers for traffic distribution
   - Deploy multiple relay instances
   - Configure automatic failover and recovery

4. **Log Management**
   - Configure appropriate log levels
   - Use centralized log collection (e.g., ELK Stack)
   - Regular log cleanup

## Troubleshooting

### Common Issues

#### 1. Continuous Memory Growth

- **Cause**: RocksDB cache not releasing in time
- **Solution**: Adjust RocksDB configuration, increase cache pressure

#### 2. High Event Processing Latency

- **Cause**: Batch size too small or insufficient relay connections
- **Solution**: Increase `batch_size` or connect to more relays

#### 3. Low Deduplication Efficiency

- **Cause**: Bloom filter capacity insufficient
- **Solution**: Increase `bloom_capacity` or `lru_size`

#### 4. WebSocket Connection Drops

- **Cause**: Downstream client network instability
- **Solution**: Increase heartbeat detection, implement automatic reconnection

### Debug Logging

```bash
# Enable debug logging
RUST_LOG=iso_relayer=debug cargo run --release

# View specific module logs
RUST_LOG=iso_relayer::core::relay_pool=trace cargo run --release

# Save logs to file
RUST_LOG=iso_relayer=info cargo run --release > relay.log 2>&1
```

## Development Guide

### Code Standards

- Follow official Rust coding conventions
- Use `cargo fmt` for code formatting
- Use `cargo clippy` for code quality checks

```bash
make fmt    # Format code
make lint   # Check code
```

### Adding New Features

1. Create new file in corresponding module directory
2. Declare module in `mod.rs`
3. Implement feature and write unit tests
4. Run `cargo test` to verify

### Performance Optimization

- Use `cargo bench` for benchmarking
- Use `perf` or Flamegraph for performance analysis
- Focus on zero-copy and async operation optimization

## Contributing

Contributions are welcome! Please ensure:

1. Code passes `cargo clippy` checks
2. Code conforms to `cargo fmt` formatting
3. Add appropriate unit tests
4. Update relevant documentation

## Contact & Contributors

[Project maintainer & contributors contact information]

![Contributors](https://contrib.rocks/image?repo=ai-chen2050/iso-relayer)

## License

[MIT](./LICENSE.MD)