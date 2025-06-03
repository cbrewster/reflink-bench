# Reflink + Write Benchmark Suite

A comprehensive performance benchmark suite comparing XFS vs btrfs reflink + write operations, with a focus on concurrent workload performance and contention analysis. Tests both reflink creation and copy-on-write performance.

## Features

- 🚀 **Filesystem Performance Comparison**: Direct comparison between XFS and btrfs reflink + write operations
- ⚡ **Concurrency Testing**: Tests with 1-128 concurrent threads to identify contention issues
- 📝 **Copy-on-Write Testing**: Each reflink is followed by a write to trigger CoW behavior
- 📊 **Detailed Reporting**: Performance metrics, throughput analysis, and contention ratios
- 🔧 **Configurable Workloads**: Adjustable file sizes, operation counts, and filesystem sizes
- 💾 **Loopback Filesystems**: Uses loopback devices for isolated testing

## Quick Start

### Using Nix Flakes

```bash
# Enter the development shell with all required tools
nix develop

# Run the benchmark suite with default settings
cargo run -- run

# Run with custom parameters
cargo run -- run --file-size-mb 500 --reflink-count 2000 --fs-size-gb 4

# Save results to JSON
cargo run -- run --output results.json

# Clean up any leftover filesystem artifacts
cargo run -- cleanup
```

### Manual Setup

Ensure you have the following tools installed:
- Rust toolchain (cargo, rustc)
- XFS utilities (xfsprogs)
- btrfs utilities (btrfs-progs)
- Loop device utilities (util-linux)

## Usage

### Basic Benchmark

```bash
# Run comparison with default settings:
# - 100MB files
# - 1000 reflinks per filesystem
# - 2GB filesystem images
cargo run -- run
```

### Custom Benchmark

```bash
# Test with larger files and more operations
cargo run -- run \
  --file-size-mb 1000 \
  --reflink-count 5000 \
  --fs-size-gb 10 \
  --output benchmark-results.json
```

### Cleanup

```bash
# Remove any leftover filesystem images and mounts
cargo run -- cleanup
```

## What It Tests

### Sequential Performance
- Creates a source file of specified size
- Measures time to create N reflinks sequentially, each followed by a 4KB write
- Calculates throughput (MB/s) and operations per second
- Tests both reflink creation and copy-on-write performance

### Concurrent Performance
- Tests with increasing thread counts (1, 2, 4, 8, 16, 32, 64, 128)
- Each operation: reflink + write to trigger CoW
- Measures contention effects as concurrency increases
- Identifies optimal concurrency levels for each filesystem

### Metrics Reported

- **Total Duration**: Time to complete all reflink operations
- **Throughput**: MB/s based on total data processed
- **Operations/Second**: Number of reflinks created per second
- **Contention Ratios**: Performance degradation at high concurrency
- **Comparative Analysis**: Head-to-head filesystem comparison

## Example Output

```
🚀 Starting reflink benchmark suite
File size: 100MB, Reflink count: 1000, FS size: 2GB

📊 Testing XFS filesystem...
📁 Creating source file...
🔗 Running sequential reflink benchmark...
⚡ Running concurrent reflink benchmarks...
  Testing with 1 concurrent threads...
  Testing with 2 concurrent threads...
  ...

📊 Testing btrfs filesystem...
...

📊 FILESYSTEM COMPARISON RESULTS
================================
┌────────────┬──────────────────┬──────────┬──────────────┬──────────────────┬─────────────────────┬───────────┐
│ Filesystem │ File Size (MB)   │ Reflinks │ Total Time   │ Avg per Reflink  │ Throughput (MB/s)   │ Ops/sec   │
├────────────┼──────────────────┼──────────┼──────────────┼──────────────────┼─────────────────────┼───────────┤
│ XFS        │ 100             │ 1000     │ 2.45s        │ 2.45ms           │ 40816.33            │ 408.16    │
│ btrfs      │ 100             │ 1000     │ 3.12s        │ 3.12ms           │ 32051.28            │ 320.51    │
└────────────┴──────────────────┴──────────┴──────────────┴──────────────────┴─────────────────────┴───────────┘

⚡ CONCURRENCY PERFORMANCE ANALYSIS
==================================
...

🏁 PERFORMANCE SUMMARY
=====================
Sequential Performance:
  🏆 XFS is 1.3x faster than btrfs
```

## Architecture

The benchmark suite consists of several modules:

- **`filesystem.rs`**: Manages loopback filesystem creation, formatting, and mounting
- **`benchmark.rs`**: Implements reflink performance testing with concurrency support
- **`results.rs`**: Handles result analysis, comparison, and reporting
- **`main.rs`**: CLI interface and orchestration

## Requirements

- Linux system with loop device support
- Root privileges (for filesystem mounting)
- XFS and btrfs kernel support
- Sufficient disk space for filesystem images

## Limitations

- Requires root privileges for mounting filesystems
- Performance may vary based on underlying storage
- Results are specific to the test environment and workload patterns

## Contributing

Contributions welcome! Areas for improvement:

- Additional filesystem support (ext4, ZFS, etc.)
- More sophisticated workload patterns
- Network filesystem testing
- Performance regression tracking