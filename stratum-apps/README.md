# Stratum Apps

Complete Stratum V2 application development kit - all utilities in one crate.

## Overview

`stratum-apps` is a unified crate that provides all the utilities needed for building Stratum V2 applications.

## Architecture

This crate is organized into several main modules:

- **`network_helpers`** - High-level networking utilities (from `network_helpers_sv2`)
- **`config_helpers`** - Configuration management helpers (from `config_helpers_sv2`)  
- **`payout`** - Shared payout-mode parsing and coinbase-output distribution helpers
- **`fallback_coordinator`** - Runtime fallback cancellation and acknowledgement helpers
- **`rpc`** - RPC utilities with custom serializable types (from `rpc_sv2`) - *feature-gated*

The crate also re-exports `stratum-core`, the central hub for the Stratum V2 ecosystem that provides a cohesive API for all low-level protocol functionality.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
stratum-apps = { version = "0.4.0", features = ["pool"] }
```

Basic usage:

```rust
use stratum_apps::{network_helpers, config_helpers};

// For RPC functionality (when rpc feature is enabled)
#[cfg(feature = "rpc")]
use stratum_apps::rpc::{BlockHash, MiniRpcClient};
```

## Features

### Core Features
- `network` - Networking utilities (enabled by default)
- `fallback-coordinator` - Runtime fallback coordination helpers (enabled by default)
- `config` - Configuration helpers (enabled by default)
- `payout` - Shared payout-mode parsing and coinbase-output distribution helpers (optional)
- `rpc` - RPC utilities with custom serializable types (optional)
  - Provides `Hash`, `BlockHash`, `Amount` types with proper JSON serialization
  - `MiniRpcClient` for Bitcoin RPC communication
- `monitoring` - HTTP and Prometheus monitoring helpers (optional)
- `std` - Standard-library support for key and random utilities (enabled by default)
- `core` - Re-export and enable `stratum-core`

### Protocol Features
- `sv1` - Enable SV1 protocol support (includes translation utilities)
- `with_buffer_pool` - Enable buffer pooling for better performance

### Role-Specific Bundles
- `pool` - Pool application helpers, including networking, config, buffer pooling, core protocol types, and payout helpers
- `jd_client` - Job Declaration Client helpers, including networking, fallback coordination, config, buffer pooling, and core protocol types
- `jd_server` - Job Declaration Server config helpers
- `translator` - Translator Proxy helpers, including networking, fallback coordination, config, SV1 translation, buffer pooling, and payout helpers
- `mining_device` - Mining device config helpers

Third-party applications that only need payout parsing or verification can use the smaller feature
set:

```toml
[dependencies]
stratum-apps = { version = "0.4.0", default-features = false, features = ["payout"] }
```

## Usage Examples

### Pool Application

```toml
[dependencies]
stratum-apps = { version = "0.4.0", features = ["pool"] }
```

```rust
use stratum_apps::{network_helpers, config_helpers};

// Use networking
let connection = network_helpers::Connection::new(stream, HandshakeRole::Responder).await?;

// Use configuration
let config: PoolConfig = config_helpers::parse_config("pool.toml")?;
```

### JD Server Application

```toml
[dependencies]
stratum-apps = { version = "0.4.0", features = ["jd_server", "rpc"] }
```

```rust
use stratum_apps::{config_helpers, rpc};

// RPC functionality with custom types
use stratum_apps::rpc::{BlockHash, MiniRpcClient};

// Configuration helpers plus RPC server utilities with proper serialization
```
