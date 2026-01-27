// src/engine/memory.rs
//
// Container memory limit detection for smart concurrency control.
//
// This module detects container memory limits from cgroup v1/v2 to automatically
// adjust thread pool size and prevent OOM kills in constrained environments.

use crate::engine::pipeline::calc_resize_dimensions;
use crate::ops::{Operation, OutputFormat, ResizeFit};
use image::ImageFormat;
use parking_lot::{Condvar, Mutex};
#[cfg(feature = "napi")]
use std::fs;
use std::io::Cursor;
use std::sync::{Arc, OnceLock};

/// Estimated memory per image operation (in bytes)
/// 100MB keeps backwards compatibility for fallback paths; dynamic estimates are preferred.
pub const ESTIMATED_MEMORY_PER_OPERATION: u64 = 100 * 1024 * 1024; // 100MB per operation (conservative)

/// Minimum memory to reserve for system and other processes (in bytes)
const MIN_RESERVED_MEMORY: u64 = 64 * 1024 * 1024; // 64MB for tiny containers
const MAX_RESERVED_MEMORY: u64 = 512 * 1024 * 1024; // cap for large hosts

/// Lower bound for any estimate to avoid zero-ish weights
const MIN_ESTIMATE_BYTES: u64 = 24 * 1024 * 1024; // 24MB

/// Overhead for decode/temporary buffers (heuristic)
const DECODE_OVERHEAD_BYTES: u64 = 8 * 1024 * 1024;
const FILTER_OVERHEAD_BYTES: u64 = 4 * 1024 * 1024;

/// Default bytes-per-pixel assumptions per format (decoded)
const BPP_JPEG: u64 = 3; // YCbCr → RGB
const BPP_PNG: u64 = 4; // favor safety (alpha)
const BPP_WEBP: u64 = 4;
const BPP_AVIF: u64 = 4;
const BPP_UNKNOWN: u64 = 4;

/// Minimum safe concurrency when memory is very constrained
#[allow(dead_code)]
const MIN_SAFE_CONCURRENCY: usize = 1;

/// Maximum safe concurrency based on memory (even if CPU allows more)
const MAX_MEMORY_BASED_CONCURRENCY: usize = 16;

/// Fallback memory capacity when detection fails (aligned with previous conservative limit)
const FALLBACK_SEMAPHORE_CAPACITY: u64 =
    ESTIMATED_MEMORY_PER_OPERATION * MAX_MEMORY_BASED_CONCURRENCY as u64;

/// In-memory weighted semaphore for byte-based backpressure
#[derive(Debug)]
pub struct WeightedSemaphore {
    capacity: u64,
    state: Mutex<u64>, // available bytes
    cvar: Condvar,
}

#[derive(Debug)]
pub struct MemoryPermit {
    sem: Arc<WeightedSemaphore>,
    weight: u64,
}

impl WeightedSemaphore {
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            state: Mutex::new(capacity),
            cvar: Condvar::new(),
        }
    }

    pub fn acquire(self: &Arc<Self>, weight: u64) -> MemoryPermit {
        let mut available = self.state.lock();
        // clamp absurd weights to capacity to avoid deadlock
        let need = weight.min(self.capacity);
        while *available < need {
            self.cvar.wait(&mut available);
        }
        *available -= need;
        MemoryPermit {
            sem: Arc::clone(self),
            weight: need,
        }
    }

    fn release(&self, weight: u64) {
        let mut available = self.state.lock();
        let freed = (*available).saturating_add(weight).min(self.capacity);
        *available = freed;
        // notify_all: When waiters have heterogeneous weights, notify_one can cause starvation.
        // Benchmarks showed wake spikes are acceptable, so we wake all waiters and prioritize
        // fairness through immediate re-contention.
        self.cvar.notify_all();
    }
}

impl Drop for MemoryPermit {
    fn drop(&mut self) {
        self.sem.release(self.weight);
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderEstimate {
    pub width: u32,
    pub height: u32,
    pub format: Option<ImageFormat>,
}

fn bytes_for_image(width: u32, height: u32, bytes_per_pixel: u64) -> u64 {
    let pixels = width as u64 * height as u64;
    pixels.saturating_mul(bytes_per_pixel)
}

fn default_bpp(format: Option<ImageFormat>) -> u64 {
    match format {
        Some(ImageFormat::Jpeg) => BPP_JPEG,
        Some(ImageFormat::Png) => BPP_PNG,
        Some(ImageFormat::WebP) => BPP_WEBP,
        Some(ImageFormat::Avif) => BPP_AVIF,
        _ => BPP_UNKNOWN,
    }
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

fn calc_cover_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: u32,
    target_h: u32,
) -> (u32, u32) {
    if orig_w == 0 || orig_h == 0 {
        return (target_w.max(1), target_h.max(1));
    }
    let scale_w = target_w as f64 / orig_w as f64;
    let scale_h = target_h as f64 / orig_h as f64;
    let scale = scale_w.max(scale_h);
    let resize_w = ((orig_w as f64 * scale).ceil() as u32).max(1);
    let resize_h = ((orig_h as f64 * scale).ceil() as u32).max(1);
    (resize_w, resize_h)
}

fn project_operation(dims: (u32, u32), current_bpp: u64, op: &Operation) -> ((u32, u32), u64, u64) {
    match op {
        Operation::Resize { width, height, fit } => {
            let target = (
                width.unwrap_or(dims.0).max(1),
                height.unwrap_or(dims.1).max(1),
            );
            match fit {
                ResizeFit::Fill => ((target.0, target.1), 4, FILTER_OVERHEAD_BYTES),
                ResizeFit::Inside => {
                    let (w, h) = calc_resize_dimensions(dims.0, dims.1, *width, *height);
                    ((w, h), 4, FILTER_OVERHEAD_BYTES)
                }
                ResizeFit::Cover => {
                    let (resize_w, resize_h) =
                        calc_cover_resize_dimensions(dims.0, dims.1, target.0, target.1);
                    // Peak occurs after resize before crop; use larger of resize and target
                    // to model intermediate buffer.
                    let resize_bytes = bytes_for_image(resize_w, resize_h, 4);
                    let target_bytes = bytes_for_image(target.0, target.1, 4);
                    let overhead = FILTER_OVERHEAD_BYTES;
                    // Return final dims (after crop), but include resize_bytes in peak.
                    (
                        (target.0, target.1),
                        4,
                        overhead.saturating_add(resize_bytes.saturating_sub(target_bytes)),
                    )
                }
            }
        }
        Operation::Extract {
            width,
            height,
            fit,
            crop_width,
            crop_height,
            ..
        } => {
            let target_resize = (
                width.unwrap_or(dims.0).max(1),
                height.unwrap_or(dims.1).max(1),
            );
            let (resize_w, resize_h) = match fit {
                ResizeFit::Fill => target_resize,
                ResizeFit::Inside => calc_resize_dimensions(dims.0, dims.1, *width, *height),
                ResizeFit::Cover => {
                    calc_cover_resize_dimensions(dims.0, dims.1, target_resize.0, target_resize.1)
                }
            };
            let final_w = (*crop_width).max(1).min(resize_w);
            let final_h = (*crop_height).max(1).min(resize_h);
            ((final_w, final_h), 4, FILTER_OVERHEAD_BYTES)
        }
        Operation::Crop { width, height, .. } => {
            let w = (*width).max(1).min(dims.0);
            let h = (*height).max(1).min(dims.1);
            ((w, h), current_bpp, FILTER_OVERHEAD_BYTES / 2)
        }
        Operation::Rotate { degrees } => {
            let rotated = matches!(degrees.rem_euclid(360), 90 | 270);
            let next_dims = if rotated { (dims.1, dims.0) } else { dims };
            (next_dims, current_bpp, FILTER_OVERHEAD_BYTES)
        }
        Operation::FlipH | Operation::FlipV => (dims, current_bpp, FILTER_OVERHEAD_BYTES / 2),
        Operation::Brightness { .. } | Operation::Contrast { .. } => {
            (dims, current_bpp.max(3), FILTER_OVERHEAD_BYTES / 2)
        }
        Operation::AutoOrient { orientation } => {
            let rotated = matches!(orientation, 5 | 6 | 7 | 8);
            let next_dims = if rotated { (dims.1, dims.0) } else { dims };
            (next_dims, current_bpp, FILTER_OVERHEAD_BYTES)
        }
        Operation::Grayscale => (dims, current_bpp.max(3), FILTER_OVERHEAD_BYTES / 2),
        Operation::ColorSpace { .. } => (dims, 3, FILTER_OVERHEAD_BYTES / 2),
    }
}

fn estimate_memory_from_dimensions_with_context(
    width: u32,
    height: u32,
    format: Option<ImageFormat>,
    ops: &[Operation],
    output_format: Option<&OutputFormat>,
) -> u64 {
    // Deterministic model: Calculate peak memory directly from input pixel count × BPP
    // and pipeline intermediate buffer count. No learning or observation-based corrections.
    let mut current_dims = (width, height);
    let mut current_bpp = default_bpp(format);

    let mut peak = bytes_for_image(current_dims.0, current_dims.1, current_bpp)
        .saturating_add(DECODE_OVERHEAD_BYTES);
    let mut current_bytes = bytes_for_image(current_dims.0, current_dims.1, current_bpp);

    for op in ops {
        let (next_dims, next_bpp, op_overhead) = project_operation(current_dims, current_bpp, op);
        let next_bytes = bytes_for_image(next_dims.0, next_dims.1, next_bpp);
        let op_peak = current_bytes
            .saturating_add(next_bytes)
            .saturating_add(op_overhead);
        peak = peak.max(op_peak);
        current_dims = next_dims;
        current_bpp = next_bpp;
        current_bytes = next_bytes;
    }

    let output_bpp = match output_format {
        Some(OutputFormat::Jpeg { .. }) => BPP_JPEG,
        Some(OutputFormat::Png)
        | Some(OutputFormat::WebP { .. })
        | Some(OutputFormat::Avif { .. }) => 4,
        None => current_bpp,
    };
    let output_bytes = bytes_for_image(current_dims.0, current_dims.1, output_bpp);
    peak = peak.max(current_bytes.saturating_add(output_bytes / 4));

    peak.max(MIN_ESTIMATE_BYTES)
}

/// Simple wrapper for callers without format/ops context (kept for compatibility in tests)
#[cfg(test)]
#[allow(dead_code)]
pub fn estimate_memory_from_dimensions(width: u32, height: u32) -> u64 {
    estimate_memory_from_dimensions_with_context(width, height, None, &[], None)
}

/// Lightweight header parse; returns None if dimensions can't be read.
pub fn estimate_memory_from_header(
    bytes: &[u8],
    ops: &[Operation],
    output_format: Option<&OutputFormat>,
) -> Option<u64> {
    parse_header(bytes).map(|header| {
        estimate_memory_from_dimensions_with_context(
            header.width,
            header.height,
            header.format,
            ops,
            output_format,
        )
    })
}

/// Parse width/height/format from input bytes without full decode.
pub fn parse_header(bytes: &[u8]) -> Option<HeaderEstimate> {
    let cursor = Cursor::new(bytes);
    if let Ok(reader) = image::ImageReader::new(cursor).with_guessed_format() {
        let format = reader.format();
        if let Ok((w, h)) = reader.into_dimensions() {
            return Some(HeaderEstimate {
                width: w,
                height: h,
                format,
            });
        }
    }
    None
}

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
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok();
    let mount = mountinfo
        .as_deref()
        .and_then(parse_cgroup2_mount_point)
        .unwrap_or_else(|| CgroupMount {
            mount_point: "/sys/fs/cgroup".to_string(),
            root: "/".to_string(),
        });

    let rel_path = fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|c| parse_cgroup2_relative_path(&c))
        .unwrap_or_default();

    let rel = strip_mount_root(&mount.root, &rel_path);
    let path = join_mount_rel_file(&mount.mount_point, &rel, "memory.max");
    if let Ok(content) = fs::read_to_string(&path) {
        let trimmed = content.trim();
        if trimmed == "max" {
            return None;
        }
        if let Ok(memory) = trimmed.parse::<u64>() {
            return Some(memory);
        }
    }
    None
}

/// Detects memory limit from cgroup v1
#[cfg(feature = "napi")]
fn detect_cgroup_v1_memory() -> Option<u64> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok();
    let mount = mountinfo
        .as_deref()
        .and_then(|m| parse_cgroup1_mount_point(m, "memory"))
        .unwrap_or_else(|| CgroupMount {
            mount_point: "/sys/fs/cgroup/memory".to_string(),
            root: "/".to_string(),
        });

    let rel_path = fs::read_to_string("/proc/self/cgroup")
        .ok()
        .as_deref()
        .and_then(parse_cgroup1_memory_relative_path)
        .unwrap_or_default();

    let rel = strip_mount_root(&mount.root, &rel_path);
    let path = join_mount_rel_file(&mount.mount_point, &rel, "memory.limit_in_bytes");

    if let Ok(content) = fs::read_to_string(&path) {
        let trimmed = content.trim();
        if let Ok(memory) = trimmed.parse::<u64>() {
            // Very large values (like 2^63-1) usually mean "no limit"
            if memory > 1_000_000_000_000_000 {
                return None; // No limit, fall back to system memory
            }
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
        if let Ok(output) = Command::new("sysctl").arg("-n").arg("hw.memsize").output() {
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

#[cfg(feature = "napi")]
struct CgroupMount {
    mount_point: String,
    root: String,
}

#[cfg(feature = "napi")]
fn parse_cgroup2_mount_point(mountinfo: &str) -> Option<CgroupMount> {
    for line in mountinfo.lines() {
        // fields: id parent major:minor root mountpoint opts ... - fstype ...
        // Example: 36 27 0:31 / /sys/fs/cgroup rw,relatime - cgroup2 cgroup2 rw
        let mut parts = line.split(" - ");
        let pre = parts.next()?;
        let post = parts.next()?;
        if !post.contains("cgroup2") {
            continue;
        }
        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() >= 5 {
            return Some(CgroupMount {
                root: pre_fields[3].to_string(),
                mount_point: pre_fields[4].to_string(),
            });
        }
    }
    None
}

#[cfg(feature = "napi")]
fn parse_cgroup1_mount_point(mountinfo: &str, controller: &str) -> Option<CgroupMount> {
    for line in mountinfo.lines() {
        let mut parts = line.split(" - ");
        let pre = parts.next()?;
        let post = parts.next()?;
        if !(post.contains("cgroup") && post.contains(controller)) {
            continue;
        }
        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() >= 5 {
            return Some(CgroupMount {
                root: pre_fields[3].to_string(),
                mount_point: pre_fields[4].to_string(),
            });
        }
    }
    None
}

#[cfg(feature = "napi")]
fn parse_cgroup2_relative_path(content: &str) -> Option<String> {
    // Format: 0::/docker/abcd...
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let _hier = parts.next()?;
        let _controllers = parts.next()?;
        let path = parts.next()?;
        return Some(path.to_string());
    }
    None
}

#[cfg(feature = "napi")]
fn parse_cgroup1_memory_relative_path(content: &str) -> Option<String> {
    // Lines like: 5:memory:/kubepods.slice/... or 5:memory:/
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let _id = parts.next()?;
        let controllers = parts.next()?;
        if controllers.split(',').any(|c| c == "memory") {
            let path = parts.next().unwrap_or("");
            return Some(path.to_string());
        }
    }
    None
}

#[cfg(feature = "napi")]
fn strip_mount_root(root: &str, rel: &str) -> String {
    if root == "/" {
        return rel.to_string();
    }
    let trimmed_root = root.trim_end_matches('/');
    let rel_no_leading = rel.trim_start_matches('/');
    let prefix = trimmed_root.trim_start_matches('/');
    if rel_no_leading.starts_with(prefix) {
        let stripped = rel_no_leading
            .trim_start_matches(prefix)
            .trim_start_matches('/');
        stripped.to_string()
    } else {
        rel.to_string()
    }
}

#[cfg(feature = "napi")]
fn join_mount_rel_file(mount_point: &str, rel: &str, file: &str) -> String {
    let base = mount_point.trim_end_matches('/');
    if rel.is_empty() {
        return format!("{}/{}", base, file);
    }
    format!("{}/{}/{}", base, rel.trim_start_matches('/'), file)
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
    use crate::ops::{Operation, OutputFormat, ResizeFit};
    use std::sync::Arc;

    #[test]
    fn test_reserved_memory_bounds_and_percent() {
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
        assert!(reserved >= 100 * 1024 * 1024 && reserved <= 110 * 1024 * 1024);
    }

    #[test]
    fn test_parse_cgroup2_mount_point() {
        let sample = "36 27 0:31 / /sys/fs/cgroup rw,relatime - cgroup2 cgroup2 rw\n";
        let mount = parse_cgroup2_mount_point(sample).unwrap();
        assert_eq!(mount.mount_point, "/sys/fs/cgroup");
        assert_eq!(mount.root, "/");
    }

    #[test]
    fn test_parse_cgroup1_memory_relative_path() {
        let sample =
            "5:memory:/kubepods.slice/kubepods-besteffort.slice\n9:cpu,cpuacct:/kubepods.slice\n";
        assert_eq!(
            parse_cgroup1_memory_relative_path(sample),
            Some("/kubepods.slice/kubepods-besteffort.slice".to_string())
        );
    }

    #[test]
    fn test_strip_mount_root_removes_duplicate() {
        let root = "/docker/12345";
        let rel = "/docker/12345/foo/bar";
        assert_eq!(strip_mount_root(root, rel), "foo/bar");
    }

    #[test]
    fn test_join_mount_rel_file_handles_empty_rel() {
        let path = join_mount_rel_file("/sys/fs/cgroup", "", "memory.max");
        assert_eq!(path, "/sys/fs/cgroup/memory.max");
    }

    #[test]
    fn test_parse_cgroup2_relative_path() {
        let sample = "0::/docker/abcd\n";
        assert_eq!(
            parse_cgroup2_relative_path(sample),
            Some("/docker/abcd".to_string())
        );
    }

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

    #[test]
    fn test_weighted_semaphore_acquire_release() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        let permit = sem.acquire(60);
        {
            let remaining = *sem.state.lock();
            assert_eq!(remaining, 40);
        }
        drop(permit);
        let remaining = *sem.state.lock();
        assert_eq!(remaining, 100);
    }

    #[test]
    fn test_estimate_memory_from_dimensions_non_zero() {
        let est = estimate_memory_from_dimensions(10, 10);
        assert!(est >= MIN_ESTIMATE_BYTES);
    }

    #[test]
    fn test_format_specific_estimate_differs() {
        let ops: Vec<Operation> = Vec::new();
        let jpeg_est = estimate_memory_from_dimensions_with_context(
            4000,
            4000,
            Some(ImageFormat::Jpeg),
            &ops,
            Some(&OutputFormat::Jpeg {
                quality: 80,
                fast_mode: false,
            }),
        );
        let png_est = estimate_memory_from_dimensions_with_context(
            4000,
            4000,
            Some(ImageFormat::Png),
            &ops,
            Some(&OutputFormat::Png),
        );
        assert!(png_est > jpeg_est);
    }

    #[test]
    fn test_cover_resize_accounts_intermediate() {
        let ops = vec![Operation::Resize {
            width: Some(1000),
            height: Some(1000),
            fit: ResizeFit::Cover,
        }];
        let est = estimate_memory_from_dimensions_with_context(100, 10_000, None, &ops, None);
        let resize_bytes = bytes_for_image(1000, 10_000, 4);
        assert!(est >= resize_bytes);
    }
}

// Tests that run when `napi` feature is disabled (the CI coverage path uses `--no-default-features`).
#[cfg(all(test, not(feature = "napi")))]
mod non_napi_tests {
    use super::*;
    use crate::ops::{Operation, OutputFormat, ResizeFit};
    use image::{ImageBuffer, ImageFormat, Rgba};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn weighted_semaphore_wakes_waiter_after_drop() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        let (tx_started, rx_started) = std::sync::mpsc::channel();
        let (tx_done, rx_done) = std::sync::mpsc::channel();

        // Hold full capacity so the spawned thread must block.
        let permit = sem.acquire(100);

        let sem_wait = Arc::clone(&sem);
        let handle = thread::spawn(move || {
            tx_started.send(()).unwrap();
            let _permit = sem_wait.acquire(10); // will block until capacity is released
            tx_done.send(()).unwrap();
        });

        // Wait until the waiter has started and is blocked inside acquire.
        rx_started.recv_timeout(Duration::from_secs(1))
            .expect("waiter should signal start");
        drop(permit); // release full capacity

        rx_done.recv_timeout(Duration::from_secs(1))
            .expect("waiter should acquire after release");
        handle.join().unwrap();
    }

    #[test]
    fn estimate_memory_from_header_returns_minimum_for_small_png() {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let estimate =
            estimate_memory_from_header(&bytes, &[], Some(&OutputFormat::Png)).unwrap();
        assert!(estimate >= MIN_ESTIMATE_BYTES);
    }

    #[test]
    fn cover_resize_estimate_grows_with_target() {
        let ops = vec![Operation::Resize {
            width: Some(200),
            height: Some(200),
            fit: ResizeFit::Cover,
        }];
        let est_small =
            estimate_memory_from_dimensions_with_context(10, 10, None, &ops, Some(&OutputFormat::Png));
        let est_large =
            estimate_memory_from_dimensions_with_context(1000, 1000, None, &ops, Some(&OutputFormat::Png));
        assert!(est_large >= est_small);
    }
}
