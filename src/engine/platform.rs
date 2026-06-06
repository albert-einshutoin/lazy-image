#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/platform.rs
//
// Centralized OS-level abstractions. All platform-specific `#[cfg]` blocks
// live here so the rest of the engine can call a uniform API.

use std::fs::File;

// ============================================================================
// Memory allocator identity
// ============================================================================

/// Name of the active global memory allocator.
#[allow(dead_code)]
pub fn allocator_name() -> &'static str {
    #[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
    {
        "jemalloc"
    }
    #[cfg(not(all(feature = "jemalloc", not(target_env = "msvc"))))]
    {
        "system"
    }
}

// ============================================================================
// System memory detection
// ============================================================================

/// Detect total physical memory in bytes.
///
/// This is the raw OS-level detection only (not container-aware).
/// For container-aware detection, use `memory::detect_available_memory()`
/// which checks cgroup limits first and falls back to this function.
#[allow(dead_code)] // Used by memory.rs when napi feature is enabled
pub fn detect_system_memory() -> Option<u64> {
    detect_system_memory_impl()
}

#[cfg(target_os = "linux")]
fn detect_system_memory_impl() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(kb_str) = line.split_whitespace().nth(1) {
                if let Ok(kb) = kb_str.parse::<u64>() {
                    return Some(kb * 1024);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detect_system_memory_impl() -> Option<u64> {
    use std::process::Command;
    let output = Command::new("sysctl")
        .arg("-n")
        .arg("hw.memsize")
        .output()
        .ok()?;
    let memory_str = String::from_utf8(output.stdout).ok()?;
    memory_str.trim().parse::<u64>().ok()
}

#[cfg(target_os = "windows")]
fn detect_system_memory_impl() -> Option<u64> {
    // Use Windows API: GetPhysicallyInstalledSystemMemory (kernel32.dll)
    // Returns total physical memory in kilobytes. Available on Vista+.
    extern "system" {
        fn GetPhysicallyInstalledSystemMemory(total_memory_in_kilobytes: *mut u64) -> i32;
    }
    let mut total_kb: u64 = 0;
    // SAFETY: `total_kb` is a properly sized, stack-allocated `u64` initialized
    // to zero. We pass its address as the sole out-parameter, which
    // `GetPhysicallyInstalledSystemMemory` expects to be a valid, writable
    // pointer to a `u64`. The pointer is live for the entire duration of the
    // call. The return value is checked before the written value is used.
    unsafe {
        if GetPhysicallyInstalledSystemMemory(&mut total_kb) != 0 {
            Some(total_kb * 1024) // KB → bytes
        } else {
            None
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_system_memory_impl() -> Option<u64> {
    None
}

// ============================================================================
// Advisory file locking
// ============================================================================

/// Try to acquire a non-blocking shared advisory lock on a file.
/// Returns true if the lock was acquired, false otherwise.
///
/// - **Unix**: `flock(LOCK_SH | LOCK_NB)` — cooperative lock, prevents
///   modification by processes that also use `flock`.
/// - **Windows**: mmap inherently prevents file modification at the OS level,
///   so this always returns true.
/// - **Other**: Returns true (no locking available, proceed optimistically).
#[allow(dead_code)] // Used by io.rs when napi feature is enabled
pub fn try_shared_lock(file: &File) -> bool {
    try_shared_lock_impl(file)
}

#[cfg(unix)]
fn try_shared_lock_impl(file: &File) -> bool {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    // SAFETY: `fd` is a valid open file descriptor obtained from a live `&File`
    // via `AsRawFd::as_raw_fd()`, which guarantees the descriptor remains valid
    // for the lifetime of `file`. `LOCK_SH | LOCK_NB` are the only flags
    // passed; no pointers are involved. The return value is checked by the
    // caller to determine whether the lock was acquired.
    unsafe { libc::flock(fd, libc::LOCK_SH | libc::LOCK_NB) == 0 }
}

#[cfg(not(unix))]
fn try_shared_lock_impl(_file: &File) -> bool {
    true
}

// ============================================================================
// Process resource usage
// ============================================================================

/// Process resource usage snapshot.
#[derive(Clone, Copy, Debug)]
pub struct ResourceUsage {
    /// User + system CPU time in seconds.
    pub cpu_time: f64,
    /// Peak resident set size in bytes.
    pub memory_rss: u64,
}

/// Get current process resource usage (CPU time and peak RSS).
/// Returns `None` on unsupported platforms or if the syscall fails.
#[allow(dead_code)] // Used by tasks.rs when napi feature is enabled
pub fn get_resource_usage() -> Option<ResourceUsage> {
    get_resource_usage_impl()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
fn get_resource_usage_impl() -> Option<ResourceUsage> {
    use libc::{getrusage, rusage, RUSAGE_SELF};
    use std::mem;

    // SAFETY: `usage` is initialized via `mem::zeroed()`, which is valid for
    // `libc::rusage` (a plain-old-data C struct with no invalid bit patterns).
    // We pass `RUSAGE_SELF` as the `who` argument (a well-defined constant) and
    // a mutable pointer to the zeroed struct as the out-parameter, which
    // `getrusage` requires to be a valid, writable pointer to a `rusage`. The
    // pointer is live for the entire duration of the call. The return value is
    // checked before any fields of `usage` are read.
    unsafe {
        let mut usage: rusage = mem::zeroed();
        if getrusage(RUSAGE_SELF, &mut usage) != 0 {
            return None;
        }
        let cpu_time = usage.ru_utime.tv_sec as f64
            + usage.ru_utime.tv_usec as f64 / 1_000_000.0
            + usage.ru_stime.tv_sec as f64
            + usage.ru_stime.tv_usec as f64 / 1_000_000.0;

        // On Linux ru_maxrss is in KB; on macOS/FreeBSD it's in bytes
        #[cfg(target_os = "linux")]
        let memory_rss = usage.ru_maxrss as u64 * 1024;
        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        let memory_rss = usage.ru_maxrss as u64;

        Some(ResourceUsage {
            cpu_time,
            memory_rss,
        })
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
fn get_resource_usage_impl() -> Option<ResourceUsage> {
    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_name_returns_known_value() {
        let name = allocator_name();
        assert!(
            name == "jemalloc" || name == "system",
            "unexpected allocator name: {name}"
        );
    }

    #[test]
    fn detect_system_memory_returns_some_on_supported_os() {
        // This test verifies detection works on the current platform
        // (CI runs on Linux and macOS where it should succeed)
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let mem = detect_system_memory();
            assert!(mem.is_some(), "memory detection should succeed on this OS");
            let bytes = mem.unwrap();
            // Sanity: at least 256 MB, at most 64 TB
            assert!(
                bytes >= 256 * 1024 * 1024,
                "detected memory too small: {bytes}"
            );
            assert!(
                bytes <= 64 * 1024 * 1024 * 1024 * 1024u64,
                "detected memory too large: {bytes}"
            );
        }
    }

    #[test]
    fn try_shared_lock_succeeds_on_temp_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let file = tmp.as_file();
        assert!(
            try_shared_lock(file),
            "shared lock on temp file should succeed"
        );
    }

    #[test]
    fn resource_usage_returns_reasonable_values() {
        if let Some(usage) = get_resource_usage() {
            // CPU time should be non-negative
            assert!(usage.cpu_time >= 0.0, "CPU time should be non-negative");
            // RSS should be > 0 for a running process
            assert!(
                usage.memory_rss > 0,
                "RSS should be positive for running process"
            );
        }
        // On unsupported platforms, None is acceptable
    }
}
