// src/engine/memory.rs
//
// Container memory limit detection for smart concurrency control.
//
// This module detects container memory limits from cgroup v1/v2 to automatically
// adjust thread pool size and prevent OOM kills in constrained environments.

use std::fs;

/// Estimated memory per image operation (in bytes)
/// This is a conservative estimate: width × height × 4 bytes (RGBA) + encoding overhead
const ESTIMATED_MEMORY_PER_OPERATION: u64 = 100 * 1024 * 1024; // 100MB per operation (conservative)

/// Minimum memory to reserve for system and other processes (in bytes)
const RESERVED_MEMORY: u64 = 128 * 1024 * 1024; // 128MB

/// Minimum safe concurrency when memory is very constrained
const MIN_SAFE_CONCURRENCY: usize = 1;

/// Maximum safe concurrency based on memory (even if CPU allows more)
const MAX_MEMORY_BASED_CONCURRENCY: usize = 16;

/// Detects available memory from container limits or system memory
///
/// Returns available memory in bytes, or None if detection fails.
/// Falls back to system memory if not in a container.
#[cfg(feature = "napi")]
pub fn detect_available_memory() -> Option<u64> {
    // Try cgroup v2 first (newer systems)
    if let Some(memory) = detect_cgroup_v2_memory() {
        return Some(memory);
    }

    // Try cgroup v1 (older systems)
    if let Some(memory) = detect_cgroup_v1_memory() {
        return Some(memory);
    }

    // Fallback to system memory (not in container)
    detect_system_memory()
}

/// Detects memory limit from cgroup v2
#[cfg(feature = "napi")]
fn detect_cgroup_v2_memory() -> Option<u64> {
    // cgroup v2 path: /sys/fs/cgroup/memory.max
    // Note: cgroup v2 uses a unified hierarchy, so the path structure is different from v1
    let path = "/sys/fs/cgroup/memory.max";

    if let Ok(content) = fs::read_to_string(path) {
        let trimmed = content.trim();
        // "max" means no limit
        if trimmed == "max" {
            return None; // No limit, fall back to system memory
        }

        if let Ok(memory) = trimmed.parse::<u64>() {
            // cgroup v2 uses bytes
            return Some(memory);
        }
    }

    None
}

/// Detects memory limit from cgroup v1
#[cfg(feature = "napi")]
fn detect_cgroup_v1_memory() -> Option<u64> {
    // cgroup v1 path: /sys/fs/cgroup/memory/memory.limit_in_bytes
    // Note: memory.max_usage_in_bytes is the maximum usage, not the limit, so we don't use it
    let path = "/sys/fs/cgroup/memory/memory.limit_in_bytes";

    if let Ok(content) = fs::read_to_string(path) {
        let trimmed = content.trim();
        if let Ok(memory) = trimmed.parse::<u64>() {
            // Very large values (like 2^63-1) usually mean "no limit"
            if memory > 1_000_000_000_000_000 {
                return None; // No limit, fall back to system memory
            }
            // cgroup v1 uses bytes
            return Some(memory);
        }
    }

    None
}

/// Detects system memory (fallback when not in container)
#[cfg(feature = "napi")]
fn detect_system_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        // Linux: read from /proc/meminfo
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return Some(kb * 1024); // Convert KB to bytes
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: use sysctl
        use std::process::Command;
        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("hw.memsize")
            .output()
        {
            if let Ok(memory_str) = String::from_utf8(output.stdout) {
                if let Ok(memory) = memory_str.trim().parse::<u64>() {
                    return Some(memory);
                }
            }
        }
    }

    // Windows and other platforms: not implemented yet
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
            // Reserve some memory for system and other processes
            let usable = mem.saturating_sub(RESERVED_MEMORY);
            if usable < ESTIMATED_MEMORY_PER_OPERATION {
                // Very constrained: use minimum
                return MIN_SAFE_CONCURRENCY;
            }
            // Calculate how many operations can fit
            let max_ops = usable / ESTIMATED_MEMORY_PER_OPERATION;
            max_ops.min(MAX_MEMORY_BASED_CONCURRENCY as u64) as usize
        }
        None => {
            // No memory limit detected: use CPU-based concurrency
            return cpu_based_concurrency;
        }
    };

    // Take the minimum of CPU-based and memory-based concurrency
    // This ensures we don't exceed either CPU or memory limits
    memory_limit.min(cpu_based_concurrency).max(MIN_SAFE_CONCURRENCY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_memory_based_concurrency_very_constrained() {
        // 256MB container: very constrained
        let result = calculate_memory_based_concurrency(Some(256 * 1024 * 1024), 8);
        assert_eq!(result, MIN_SAFE_CONCURRENCY);
    }

    #[test]
    fn test_calculate_memory_based_concurrency_constrained() {
        // 512MB container: can fit ~3 operations (512MB - 128MB reserve = 384MB / 100MB = 3)
        let result = calculate_memory_based_concurrency(Some(512 * 1024 * 1024), 8);
        assert!(result >= MIN_SAFE_CONCURRENCY && result <= 4);
    }

    #[test]
    fn test_calculate_memory_based_concurrency_abundant() {
        // 4GB container: memory allows many operations, but CPU limits to 4
        let result = calculate_memory_based_concurrency(Some(4 * 1024 * 1024 * 1024), 4);
        assert_eq!(result, 4); // Limited by CPU
    }

    #[test]
    fn test_calculate_memory_based_concurrency_no_limit() {
        // No memory limit: use CPU-based concurrency
        let result = calculate_memory_based_concurrency(None, 8);
        assert_eq!(result, 8);
    }

    #[test]
    fn test_calculate_memory_based_concurrency_memory_limits_cpu() {
        // 1GB container with 8 CPUs: memory limits to ~8 operations, but we cap at 16
        let result = calculate_memory_based_concurrency(Some(1024 * 1024 * 1024), 8);
        assert!(result <= 8); // Limited by memory (1GB - 128MB = 896MB / 100MB = 8)
    }
}
