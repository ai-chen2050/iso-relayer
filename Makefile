.PHONY: help build dev run debug test bench clean fmt lint release docker-build docker-run docker-push install deps update

# Project information
PROJECT_NAME = iso-relayer
VERSION = 0.1.0
CARGO = cargo
DOCKER_REGISTRY = docker.io
DOCKER_IMAGE = $(DOCKER_REGISTRY)/iso-relayer:latest

# Compilation targets
TARGET_DIR = target
RELEASE_BINARY = $(TARGET_DIR)/release/$(PROJECT_NAME)
DEBUG_BINARY = $(TARGET_DIR)/debug/$(PROJECT_NAME)

# Default target
.DEFAULT_GOAL := help

# ============================================================================
# Help
# ============================================================================
help: ## Display all available commands
	@echo "=================================================================================="
	@echo "$(PROJECT_NAME) v$(VERSION) - Makefile Commands"
	@echo "=================================================================================="
	@echo ""
	@echo "Usage: make [command]"
	@echo ""
	@echo "Build commands:"
	@echo "  make build              - Build release version (optimized, recommended for production)"
	@echo "  make dev                - Build development version (debug mode, fast compile)"
	@echo "  make release            - Build release version and show binary path"
	@echo "  make deps               - Download project dependencies"
	@echo "  make update             - Update all dependencies to latest compatible version"
	@echo ""
	@echo "Run commands:"
	@echo "  make run                - Run release version"
	@echo "  make debug              - Run development version (debug mode)"
	@echo "  make run-with-config    - Run using config file"
	@echo ""
	@echo "Test and check:"
	@echo "  make test               - Run all unit tests"
	@echo "  make test-verbose       - Run tests (with detailed output)"
	@echo "  make bench              - Run benchmark tests"
	@echo "  make fmt                - Auto-format code"
	@echo "  make fmt-check          - Check code format (no modifications)"
	@echo "  make lint               - Run Clippy code check"
	@echo ""
	@echo "Clean commands:"
	@echo "  make clean              - Delete build artifacts"
	@echo "  make clean-db           - Delete RocksDB data files"
	@echo "  make clean-all          - Full cleanup (including dependencies)"
	@echo ""
	@echo "Docker commands:"
	@echo "  make docker-build       - Build Docker image"
	@echo "  make docker-run         - Run Docker container"
	@echo "  make docker-stop        - Stop Docker container"
	@echo "  make docker-logs        - View container logs"
	@echo "  make docker-push        - Push Docker image to registry"
	@echo "  make docker-clean       - Delete Docker image"
	@echo ""
	@echo "Other commands:"
	@echo "  make info               - Show project information"
	@echo "  make help               - Show this help message"
	@echo ""
	@echo "=================================================================================="
	@echo "Examples:"
	@echo "  # Quick start"
	@echo "  make build && make run"
	@echo ""
	@echo "  # Development mode"
	@echo "  make dev && make debug"
	@echo ""
	@echo "  # Docker deployment"
	@echo "  make docker-build && make docker-run"
	@echo "=================================================================================="

# ============================================================================
# Build targets
# ============================================================================
build: ## Build release version (optimized, recommended for production)
	@echo "🔨 Building release version..."
	@$(CARGO) build --release
	@echo "✅ Build complete!"
	@echo "📦 Binary location: $(RELEASE_BINARY)"

dev: ## Build development version (debug mode)
	@echo "🔨 Building development version..."
	@$(CARGO) build
	@echo "✅ Build complete!"
	@echo "📦 Binary location: $(DEBUG_BINARY)"

release: build ## Build release version and show path
	@echo "📦 Release binary:"
	@echo "   $(RELEASE_BINARY)"
	@ls -lh $(RELEASE_BINARY)

deps: ## Download and check dependencies
	@echo "📥 Downloading dependencies..."
	@$(CARGO) fetch
	@echo "✅ Dependencies downloaded!"

update: ## Update all dependencies to latest compatible version
	@echo "🔄 Updating dependencies..."
	@$(CARGO) update
	@echo "✅ Dependencies updated!"

# ============================================================================
# Run targets
# ============================================================================
run: build ## Run release version
	@echo "🚀 Running release version..."
	@$(RELEASE_BINARY)

debug: dev ## Run development version (debug mode)
	@echo "🚀 Running development version..."
	@$(DEBUG_BINARY) --config config.template.toml

run-with-config: build ## Run release version using config file
	@echo "🚀 Running with config file..."
	@if [ ! -f config.toml ]; then \
		echo "⚠️  config.toml not found, using default config"; \
	fi
	@$(RELEASE_BINARY) --config config.toml

# ============================================================================
# Test and check targets
# ============================================================================
test: ## Run all unit tests
	@echo "🧪 Running unit tests..."
	@$(CARGO) test --lib
	@echo "✅ Tests complete!"

test-verbose: ## Run tests with detailed output
	@echo "🧪 Running unit tests (verbose output)..."
	@$(CARGO) test --lib -- --nocapture --test-threads=1

test-integration: ## Run integration tests
	@echo "🧪 Running integration tests..."
	@$(CARGO) test --test '*'

bench: ## Run benchmark tests
	@echo "📊 Running benchmark tests..."
	@$(CARGO) bench
	@echo "✅ Benchmarks complete!"

fmt: ## Auto-format code
	@echo "🎨 Formatting code..."
	@$(CARGO) fmt
	@echo "✅ Code formatted!"

fmt-check: ## Check code format (no modifications)
	@echo "🔍 Checking code format..."
	@$(CARGO) fmt -- --check
	@echo "✅ Format check complete!"

lint: ## Run Clippy code check and optimization suggestions
	@echo "🔍 Running Clippy check..."
	@$(CARGO) clippy -- -D warnings
	@echo "✅ Clippy check complete!"

# ============================================================================
# Clean targets
# ============================================================================
clean: ## Delete build artifacts
	@echo "🧹 Cleaning build artifacts..."
	@$(CARGO) clean
	@echo "✅ Cleanup complete!"

clean-db: ## Delete RocksDB data files (warning: deletes all local data)
	@echo "⚠️  Deleting RocksDB data files..."
	@rm -rf data/rocksdb
	@echo "✅ RocksDB data deleted!"

clean-all: clean clean-db ## Full cleanup (including dependencies and data)
	@echo "🧹 Full cleanup..."
	@$(CARGO) clean
	@rm -rf Cargo.lock
	@echo "✅ Full cleanup complete!"

# ============================================================================
# Docker targets
# ============================================================================
docker-build: ## Build Docker image
	@echo "🐳 Building Docker image..."
	@docker build -t $(DOCKER_IMAGE) .
	@echo "✅ Docker image built!"
	@echo "📦 Image name: $(DOCKER_IMAGE)"

docker-run: docker-build ## Run Docker container
	@echo "🚀 Running Docker container..."
	@docker run -d \
		--name iso-relayer \
		-p 8080:8080 \
		-p 9090:9090 \
		-v $(PWD)/data:/app/data \
		-e RUST_LOG=iso_relayer=info \
		$(DOCKER_IMAGE)
	@echo "✅ Container started!"
	@echo "🌐 REST API: http://localhost:8080"
	@echo "📊 Prometheus: http://localhost:9090"

docker-stop: ## Stop Docker container
	@echo "⏹️  Stopping Docker container..."
	@docker stop iso-relayer || true
	@docker rm iso-relayer || true
	@echo "✅ Container stopped!"

docker-logs: ## View container logs
	@echo "📋 Container logs:"
	@docker logs -f iso-relayer

docker-push: docker-build ## Push Docker image to registry
	@echo "📤 Pushing Docker image..."
	@docker push $(DOCKER_IMAGE)
	@echo "✅ Image pushed!"

docker-clean: docker-stop ## Delete Docker image
	@echo "🧹 Deleting Docker image..."
	@docker rmi $(DOCKER_IMAGE) || true
	@echo "✅ Image deleted!"

# ============================================================================
# Information targets
# ============================================================================
info: ## Display project information
	@echo ""
	@echo "=================================================================================="
	@echo "Project Information"
	@echo "=================================================================================="
	@echo "Project name: $(PROJECT_NAME)"
	@echo "Version: $(VERSION)"
	@echo "Rust version:"
	@$(CARGO) --version
	@echo "Cargo version:"
	@$(CARGO) --version
	@echo ""
	@echo "Project statistics:"
	@wc -l src/**/*.rs | tail -1
	@echo ""
	@echo "Dependency information:"
	@$(CARGO) tree --depth 1
	@echo ""
	@echo "=================================================================================="

# ============================================================================
# Development toolchain targets
# ============================================================================
install-tools: ## Install development toolchain
	@echo "📥 Installing development tools..."
	@rustup component add rustfmt clippy
	@$(CARGO) install cargo-deny cargo-audit
	@echo "✅ Tools installed!"

security-audit: ## Check for security vulnerabilities
	@echo "🔐 Checking dependency security vulnerabilities..."
	@$(CARGO) audit
	@echo "✅ Security check complete!"

check-deps: ## Check dependency issues
	@echo "🔍 Checking dependencies..."
	@$(CARGO) deny check
	@echo "✅ Dependency check complete!"

# ============================================================================
# Performance analysis targets
# ============================================================================
perf: build ## Build performance analysis version
	@echo "📊 Building performance analysis version..."
	@$(CARGO) build --release --profile-timing
	@echo "✅ Build complete!"

# ============================================================================
# Documentation targets
# ============================================================================
doc: ## Generate Rust documentation
	@echo "📚 Generating documentation..."
	@$(CARGO) doc --no-deps --open
	@echo "✅ Documentation generated!"

# ============================================================================
# Shortcut commands
# ============================================================================
.PHONY: quick-test quick-build quick-run

quick-test: ## Quick test (skip dependency check)
	@$(CARGO) test --lib

quick-build: ## Quick build (incremental compilation)
	@$(CARGO) build --release

quick-run: quick-build ## Quick run
	@$(RELEASE_BINARY)

# ============================================================================
# Environment setup
# ============================================================================
.PHONY: setup-env

setup-env: ## Setup development environment
	@echo "🔧 Setting up development environment..."
	@cp config.template.toml config.toml 2>/dev/null || echo "config.toml already exists"
	@mkdir -p data/rocksdb
	@echo "✅ Environment setup complete!"
	@echo ""
	@echo "Next steps:"
	@echo "  1. Edit config.toml configuration file"
	@echo "  2. Run 'make build' to compile the project"
	@echo "  3. Run 'make run' to start the service"

# ============================================================================
# Verification and checks
# ============================================================================
verify: fmt-check lint test ## Complete code verification (format, lint, tests)
	@echo "✅ All verifications complete!"

# ============================================================================
# CI/CD targets
# ============================================================================
ci: verify build ## CI/CD process (verify + build)
	@echo "✅ CI/CD process complete!"

# ============================================================================
# Build time analysis
# ============================================================================
time-build: ## Show detailed build time
	@echo "⏱️  Build time analysis..."
	@$(CARGO) build --release --timings
	@echo "📊 Time statistics saved to target/cargo-timings/cargo-timing-*.html"
