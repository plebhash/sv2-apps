//! Global configuration for Translator Proxy operating mode.
//!
//! This module defines different operating modes for the Translator Proxy
//! and provides atomic accessors for setting and retrieving the current mode.
//!
//! Aggregated vs non-aggregated mode is stored in a global [`OnceLock`], which can only be set once
//! during initialization.
use std::sync::OnceLock;

/// Global atomic variable storing the current aggregation mode.
/// True if aggregated mode, false if non-aggregated mode.
pub static AGGREGATION_MODE: OnceLock<bool> = OnceLock::new();
