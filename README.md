# EasyK8s 🚀

> Simplifying Kubernetes cluster bootstrapping for SRE training and LLM development

EasyK8s is a Rust-based toolkit designed to rapidly spin up and manage Kubernetes clusters for training and testing SRE automation agents, particularly LLMs focused on Kubernetes operations.

## Overview

This project provides a streamlined way to create reproducible Kubernetes environments using k3d (k3s in Docker). It's specifically designed to:

- **Generate training data** for Kubernetes-focused LLMs
- **Test SRE automation workflows** in isolated environments
- **Rapidly prototype** cluster configurations
- **Simulate real-world scenarios** for agent training

## Features

- 🎯 **Simple API**: Minimal code to spin up multi-node clusters
- ⚡ **Fast Setup**: Leverages k3d for Docker-based Kubernetes clusters
- 🔄 **Reproducible**: Deterministic cluster creation for consistent training environments
- 🦀 **Rust-Powered**: Type-safe, performant cluster management
- 🧪 **Training-Ready**: Built specifically for LLM training workflows

## Prerequisites

Before running EasyK8s, you'll need:

- **Docker**: Container runtime for k3d
- **k3d**: Lightweight Kubernetes distribution
- **kubectl**: Kubernetes CLI tool
- **Rust**: 1.70 or higher (with Cargo)

### Quick Setup

Run the provided setup script to install dependencies:

```bash
chmod +x setup.sh
./setup.sh
```

Or install manually:

```bash
# Install Docker
sudo apt install docker.io -y

# Install k3d
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

# Verify kubectl
kubectl version --client
```

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/easyk8s.git
cd easyk8s

# Build the project
cargo build --release

# Run the cluster manager
cargo run
```

## Usage

### Basic Cluster Creation

The default configuration creates a robust test cluster:

```bash
cargo run
```

This creates:
- **3 control plane nodes** (high availability)
- **3 worker nodes** (realistic workload distribution)
- **Auto-configured kubectl** context

### Current Configuration

```rust
// Creates "rusty-cluster" with:
// - 3 server nodes (control plane)
// - 3 agent nodes (workers)
// - Automatic readiness checks
let cluster_name = "rusty-cluster";
```

### Verification

After creation, the tool automatically displays:

```bash
kubectl get nodes -o wide
```

You should see output similar to:

```
NAME                        STATUS   ROLES                  AGE   VERSION
k3d-rusty-cluster-server-0  Ready    control-plane,master   30s   v1.27.4+k3s1
k3d-rusty-cluster-server-1  Ready    control-plane,master   28s   v1.27.4+k3s1
k3d-rusty-cluster-server-2  Ready    control-plane,master   26s   v1.27.4+k3s1
k3d-rusty-cluster-agent-0   Ready    <none>                 24s   v1.27.4+k3s1
k3d-rusty-cluster-agent-1   Ready    <none>                 22s   v1.27.4+k3s1
k3d-rusty-cluster-agent-2   Ready    <none>                 20s   v1.27.4+k3s1
```

## Architecture

### Project Structure

```
k3s/
├── Cargo.toml              # Project dependencies
├── Cargo.lock              # Locked dependency versions
├── setup.sh                # Environment setup script
├── src/
│   └── main.rs             # Cluster management logic
```

### Code Architecture

The tool is built around a simple, extensible pattern:

1. **Async Runtime**: Uses Tokio for non-blocking operations
2. **Command Execution**: Wraps k3d/kubectl with error handling
3. **Health Checks**: Waits for cluster readiness before proceeding
4. **Extensibility**: Easy to add custom configurations

```rust
// Core pattern
run("k3d", &["cluster", "create", cluster_name, ...])?;
sleep(Duration::from_secs(10)).await;
run("kubectl", &["get", "nodes", "-o", "wide"])?;
```

## Use Cases for SRE Training

### 1. Incident Response Training
Create clusters with specific failure scenarios:
```rust
// Example: Create cluster with degraded nodes
// TODO: Add node failure simulation
```

### 2. Scaling Scenarios
Train agents on autoscaling decisions:
```rust
// Example: Varying worker counts
// TODO: Add dynamic scaling configurations
```

### 3. Deployment Patterns
Test different deployment strategies:
```rust
// Example: Blue-green, canary deployments
// TODO: Add workload templates
```

### 4. Resource Management
Simulate resource constraints:
```rust
// Example: Memory/CPU limits
// TODO: Add resource quota configurations
```

## Roadmap

### Phase 1: Core Functionality ✅
- [x] Basic cluster creation
- [x] Multi-node support
- [x] Health check integration
- [x] Error handling

### Phase 2: Training Features (Planned)
- [ ] **Scenario Templates**: Pre-configured cluster states
- [ ] **Workload Injection**: Automatic deployment of test applications
- [ ] **Metric Collection**: Export cluster state for training data
- [ ] **Chaos Engineering**: Integrated failure injection
- [ ] **State Snapshots**: Save/restore cluster configurations

### Phase 3: LLM Integration (Planned)
- [ ] **API Server**: REST API for cluster operations
- [ ] **Training Data Export**: Structured logs and state transitions
- [ ] **Agent Testing Framework**: Evaluate LLM decisions
- [ ] **Multi-Cluster Management**: Parallel test environments
- [ ] **Observation Hooks**: Custom telemetry for agent training

### Phase 4: Advanced Features (Future)
- [ ] **Cloud Provider Simulation**: Mock AWS/GCP/Azure resources
- [ ] **Network Policy Testing**: Complex networking scenarios
- [ ] **Security Scenarios**: RBAC, PSP, and security training
- [ ] **Cost Modeling**: Simulate resource costs for optimization training

## Customization

### Modifying Cluster Size

Edit `main.rs` to adjust node counts:

```rust
run("k3d", &[
    "cluster", "create", cluster_name,
    "--servers", "1",    // Change control plane count
    "--agents", "5",     // Change worker count
    "--wait",
])?;
```

### Adding Workloads

Uncomment or add kubectl commands:

```rust
// Deploy your training workload
run("kubectl", &["apply", "-f", "your-agent.yaml"])?;
```

### Auto-Cleanup

Enable automatic cluster deletion:

```rust
// Uncomment these lines in main.rs
println!("🧹 Cleaning up...");
run("k3d", &["cluster", "delete", cluster_name])?;
```

## Dependencies

- **anyhow** (1.0): Ergonomic error handling
- **tokio** (1.48): Async runtime with full features
- **k3d**: Lightweight Kubernetes in Docker
- **kubectl**: Kubernetes CLI

## Contributing

This is an early-stage project built for SRE training workflows. Contributions are welcome!

### Areas for Contribution

1. **Scenario Templates**: Add pre-configured cluster scenarios
2. **Workload Generators**: Create realistic application deployments
3. **Metrics Integration**: Export training data in useful formats
4. **Documentation**: Improve setup guides and examples
5. **Testing**: Add integration tests for cluster creation

### Development

```bash
# Run with logging
RUST_LOG=debug cargo run

# Run tests (when added)
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## FAQ

**Q: Why k3d instead of kind or minikube?**
A: k3d is lightweight, fast, and perfect for ephemeral test clusters. It's ideal for training scenarios where you need to spin up/down clusters frequently.

**Q: Can I use this in production?**
A: No, this is designed for training and testing only. k3d clusters are not production-ready.

**Q: How do I scale to multiple clusters?**
A: Currently supports one cluster at a time. Multi-cluster support is planned for Phase 3.

**Q: Can I integrate this with my LLM training pipeline?**
A: Absolutely! That's the goal. Export cluster state, run your agent, collect results. API integration is coming in Phase 3.

**Built with 🦀 Rust for the future of SRE automation**