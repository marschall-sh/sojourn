# Building sojourn

This document provides instructions for building sojourn from source.

## Prerequisites

### System Requirements
- Linux, macOS, or BSD system
- 4GB RAM (minimum; 8GB+ recommended for faster builds)
- ~500MB disk space for Rust toolchain and build artifacts

### Required Tools
- **Rust 1.70 or later**: https://rustup.rs/

## Installation

### 1. Install Rust

On Linux/macOS/BSD:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

On Windows: Download from https://rustup.rs/

Verify installation:
```bash
rustc --version
cargo --version
```

### 2. Clone or Extract Source

```bash
cd /path/to/sojourn
```

## Building

### Development Build (faster, larger binary)
```bash
cargo build
./target/debug/sojourn
```

### Release Build (slower compile, optimized binary)
```bash
cargo build --release
./target/release/sojourn
```

### Installation (system-wide)
```bash
cargo install --path .
sojourn  # Now available from anywhere
```

## First Run

After building, you can run sojourn:

```bash
# Using the binary directly
./target/release/sojourn

# Or after installation
sojourn
```

You'll likely need to configure it first. See Configuration below.

## Configuration

Create `~/.config/sojourn/config.toml`:

```toml
[settings]
default_user = "ubuntu"

[[inventory]]
type = "ssh_config"
path = "~/.ssh/config"
```

This minimal config will load your existing SSH hosts.

## Development Workflow

### Format Code
```bash
cargo fmt
```

### Run Linter
```bash
cargo clippy
```

### Run Tests
```bash
cargo test
```

### Build Documentation
```bash
cargo doc --open
```

### Clean Build Artifacts
```bash
cargo clean
```

## Troubleshooting Build Issues

### "couldn't compile ratatui"
- Ensure you have Rust 1.70+: `rustc --version`
- Try updating: `rustup update`
- Clean and rebuild: `cargo clean && cargo build --release`

### "Permission denied" during installation
Use a different install location:
```bash
cargo install --path . --root ~/.local
export PATH="$HOME/.local/bin:$PATH"
```

### Out of memory during build
Use a single-threaded build:
```bash
cargo build --release -j 1
```

### Network issues downloading dependencies
Try using a Rust mirror:
```bash
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'MIRROR'
[registries.crates-io]
protocol = "sparse"
MIRROR
```

## Build Options

### Cross Compilation

Build for a different target (e.g., x86_64 Linux from macOS):

```bash
# Install target
rustup target add x86_64-unknown-linux-gnu

# Build for that target
cargo build --release --target x86_64-unknown-linux-gnu
```

Available targets: `rustup target list`

### Feature Flags

Currently sojourn has no optional features, but here's the structure for future expansion:

```bash
cargo build --release --features "some_feature"
```

## Performance Notes

Build times on typical hardware:
- **Cold build** (first build, no cache): 5-10 minutes
- **Incremental build** (after changes): 10-30 seconds
- **Release build** (optimized): +2-3 minutes extra

To speed up development builds:
- Use `cargo build` (debug mode) instead of `--release` for testing
- Set `CARGO_BUILD_JOBS` to number of CPU cores
- On Linux, install `mold` or `lld` for faster linking

## Docker Build

If you don't want to install Rust locally:

```dockerfile
FROM rust:1.75

WORKDIR /app
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=0 /app/target/release/sojourn /usr/local/bin/
RUN apt-get update && apt-get install -y openssh-client && rm -rf /var/lib/apt/lists/*

ENTRYPOINT ["sojourn"]
```

Build and run:
```bash
docker build -t sojourn .
docker run -it sojourn
```

## Uninstallation

To remove sojourn:

```bash
# If installed via cargo install
cargo uninstall sojourn

# If you built it manually
rm -f ./target/release/sojourn
```

## Getting Help

If you encounter build issues:

1. Check that all prerequisites are installed
2. Run `cargo clean && cargo build` to force a full rebuild
3. Check the [Rust Book](https://doc.rust-lang.org/book/) for general Rust issues
4. Report issues on GitHub with:
   - Output of `rustc --version` and `cargo --version`
   - Full error message
   - Your OS and architecture
