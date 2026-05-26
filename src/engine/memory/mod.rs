#![cfg_attr(not(feature = "napi"), allow(dead_code, unused_imports))]

// src/engine/memory/mod.rs
//
// Container memory limit detection and weighted backpressure for concurrent
// image operations.
//
// Submodules:
//   - `cgroup`    — cgroup v1/v2 mount + memory limit parsing (napi-only)
//   - `semaphore` — fair weighted semaphore + memory permit
//   - `estimate`  — header-aware peak-memory estimation
//
// This file owns the small set of cross-cutting concerns: capacity sizing,
// the global semaphore singleton, and the public memory-detection entry
// points that decide between cgroup limits and host memory.

#[cfg(feature = "napi")]
mod cgroup;
mod estimate;
mod semaphore;

pub use estimate::{estimate_fallback_from_file_size, estimate_memory_from_header};
pub use semaphore::{MemoryPermit, WeightedSemaphore};

use std::sync::{Arc, OnceLock};

/// Estimated memory per image operation (in bytes)
/// 100MB keeps backwards compatibility for fallback paths; dynamic estimates are preferred.
pub const ESTIMATED_MEMORY_PER_OPERATION: u64 = 100 * 1024 * 1024; // 100MB per operation (conservative)

/// Minimum memory to reserve for system and other processes (in bytes)
const MIN_RESERVED_MEMORY: u64 = 64 * 1024 * 1024; // 64MB for tiny containers
const MAX_RESERVED_MEMORY: u64 = 512 * 1024 * 1024; // cap for large hosts

use estimate::MIN_ESTIMATE_BYTES;

/// Minimum safe concurrency when memory is very constrained
const MIN_SAFE_CONCURRENCY: usize = 1;

/// Maximum safe concurrency based on memory (even if CPU allows more)
const MAX_MEMORY_BASED_CONCURRENCY: usize = 16;

/// Fallback memory capacity when detection fails (aligned with previous conservative limit)
const FALLBACK_SEMAPHORE_CAPACITY: u64 =
    ESTIMATED_MEMORY_PER_OPERATION * MAX_MEMORY_BASED_CONCURRENCY as u64;

fn compute_semaphore_capacity() -> u64 {
    // Try to honor detected memory; if no detection, use fallback
    let available = detect_available_memory();
    match available {
        Some(mem) => {
            let reserved = compute_reserved_memory(mem);
            let usable = mem.saturating_sub(reserved);
            usable.max(MIN_ESTIMATE_BYTES)
        }
        None => FALLBACK_SEMAPHORE_CAPACITY,
    }
}

static GLOBAL_MEMORY_SEMAPHORE: OnceLock<Arc<WeightedSemaphore>> = OnceLock::new();

/// Get global weighted semaphore for memory backpressure
pub fn memory_semaphore() -> Arc<WeightedSemaphore> {
    GLOBAL_MEMORY_SEMAPHORE
        .get_or_init(|| Arc::new(WeightedSemaphore::new(compute_semaphore_capacity())))
        .clone()
}

/// Reserve memory for OS / runtime based on container/host limit
#[cfg(feature = "napi")]
fn compute_reserved_memory(total_bytes: u64) -> u64 {
    // reserve 5% of total, clamped to [64MB, 512MB]
    let five_percent = total_bytes / 20;
    five_percent
        .max(MIN_RESERVED_MEMORY)
        .min(MAX_RESERVED_MEMORY)
}

#[cfg(not(feature = "napi"))]
fn compute_reserved_memory(total_bytes: u64) -> u64 {
    let _ = total_bytes;
    let _ = MAX_RESERVED_MEMORY; // keep constant used in non-NAPI builds
    MIN_RESERVED_MEMORY
}

/// Detects available memory from container limits or system memory
///
/// Returns available memory in bytes, or None if detection fails.
/// Falls back to system memory if not in a container.
#[cfg(feature = "napi")]
pub fn detect_available_memory() -> Option<u64> {
    // Try cgroup v2 first (newer systems)
    if let Some(memory) = cgroup::detect_cgroup_v2_memory() {
        return Some(memory);
    }

    // Try cgroup v1 (older systems)
    if let Some(memory) = cgroup::detect_cgroup_v1_memory() {
        return Some(memory);
    }

    // Fallback to system memory (not in container)
    detect_system_memory()
}

/// Detects system memory (fallback when not in container).
/// Delegates to the centralized platform module for OS-specific detection.
#[cfg(feature = "napi")]
fn detect_system_memory() -> Option<u64> {
    super::platform::detect_system_memory()
}

/// Non-NAPI builds: skip detection and return None (use fallback)
#[cfg(not(feature = "napi"))]
pub fn detect_available_memory() -> Option<u64> {
    None
}

/// Calculates safe concurrency based on available memory
///
/// This function estimates how many concurrent image operations can safely
/// run without causing OOM kills.
///
/// # Arguments
/// * `available_memory` - Available memory in bytes (from detect_available_memory)
/// * `cpu_based_concurrency` - Concurrency based on CPU cores (from available_parallelism)
///
/// # Returns
/// Safe concurrency value (number of concurrent operations)
#[cfg(feature = "napi")]
pub fn calculate_memory_based_concurrency(
    available_memory: Option<u64>,
    cpu_based_concurrency: usize,
) -> usize {
    let memory_limit = match available_memory {
        Some(mem) => {
            let reserved = compute_reserved_memory(mem);
            let usable = mem.saturating_sub(reserved);
            if usable < MIN_ESTIMATE_BYTES {
                // Very constrained: use minimum
                return MIN_SAFE_CONCURRENCY;
            }
            // Calculate how many operations can fit
            let max_ops = usable / ESTIMATED_MEMORY_PER_OPERATION;
            max_ops.max(1).min(MAX_MEMORY_BASED_CONCURRENCY as u64) as usize
        }
        None => {
            // No memory limit detected: use CPU-based concurrency
            return cpu_based_concurrency;
        }
    };

    // Take the minimum of CPU-based and memory-based concurrency
    // This ensures we don't exceed either CPU or memory limits
    memory_limit
        .min(cpu_based_concurrency)
        .max(MIN_SAFE_CONCURRENCY)
}

#[cfg(all(test, feature = "napi"))]
mod tests {
    use super::*;

    #[test]
    fn reserved_memory_bounds_and_percent() {
        // tiny container: min 64MB
        assert_eq!(
            compute_reserved_memory(128 * 1024 * 1024),
            MIN_RESERVED_MEMORY
        );
        // huge host: capped at 512MB
        assert_eq!(
            compute_reserved_memory(64 * 1024 * 1024 * 1024),
            MAX_RESERVED_MEMORY
        );
        // 2GB -> 5% = 102.4MB -> within bounds
        let reserved = compute_reserved_memory(2 * 1024 * 1024 * 1024);
        assert!((100 * 1024 * 1024..=110 * 1024 * 1024).contains(&reserved));
    }

    #[test]
    fn calculate_memory_based_concurrency_very_constrained() {
        // 256MB container: very constrained
        let result = calculate_memory_based_concurrency(Some(256 * 1024 * 1024), 8);
        assert_eq!(result, MIN_SAFE_CONCURRENCY);
    }

    #[test]
    fn calculate_memory_based_concurrency_constrained() {
        // 512MB container: can fit ~3 operations (512MB - 128MB reserve = 384MB / 100MB = 3)
        let result = calculate_memory_based_concurrency(Some(512 * 1024 * 1024), 8);
        assert!((MIN_SAFE_CONCURRENCY..=4).contains(&result));
    }

    #[test]
    fn calculate_memory_based_concurrency_abundant() {
        // 4GB container: memory allows many operations, but CPU limits to 4
        let result = calculate_memory_based_concurrency(Some(4 * 1024 * 1024 * 1024), 4);
        assert_eq!(result, 4); // Limited by CPU
    }

    #[test]
    fn calculate_memory_based_concurrency_no_limit() {
        // No memory limit: use CPU-based concurrency
        let result = calculate_memory_based_concurrency(None, 8);
        assert_eq!(result, 8);
    }

    #[test]
    fn calculate_memory_based_concurrency_memory_limits_cpu() {
        // 1GB container with 8 CPUs: memory limits to ~8 operations, but we cap at 16
        let result = calculate_memory_based_concurrency(Some(1024 * 1024 * 1024), 8);
        assert!(result <= 8); // Limited by memory (1GB - 128MB = 896MB / 100MB = 8)
    }
}
