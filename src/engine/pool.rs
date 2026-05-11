// src/engine/pool.rs
//
// Global thread pool management for batch processing.
//
// **Architecture Decision**: We use a single global thread pool for all batch
// operations instead of creating a new pool per request. This provides:
//
// 1. **Zero allocation overhead**: No pool creation cost per batch
// 2. **Better resource utilization**: Threads are reused across operations
// 3. **Predictable performance**: Consistent thread count based on CPU cores
//
// **Thread Count Calculation**:
// - Uses std::thread::available_parallelism() to respect cgroup/CPU quota
// - Reserves UV_THREADPOOL_SIZE threads for libuv (defaults to 4) to avoid oversubscription
// - Considers memory limits for smart concurrency (see memory.rs)
// - Fallback is MIN_RAYON_THREADS when detection fails
//
// **IMPORTANT**:
// - Pool is initialized lazily on first use
// - Changes after initialization have NO effect
//
// **Benchmark Results** (see benches/benchmark.rs):
// - Global pool: ~0.5ms overhead for 100 items
// - New pool per call: ~5-10ms overhead (10-20x slower)

#[cfg(feature = "napi")]
use crate::engine::memory;
#[cfg(feature = "napi")]
use crate::error::LazyImageError;
#[cfg(all(test, feature = "napi"))]
use parking_lot::RwLock;
#[cfg(feature = "napi")]
use rayon::ThreadPool;
#[cfg(feature = "napi")]
use std::sync::{Arc, OnceLock};

/// Default libuv thread pool size (Node.js default)
#[cfg(feature = "napi")]
const DEFAULT_LIBUV_THREADPOOL_SIZE: usize = 4;

/// Maximum allowed concurrency value for processBatch()
#[cfg(feature = "napi")]
pub const MAX_CONCURRENCY: usize = 1024;

/// Minimum number of rayon threads to ensure at least some parallelism
#[cfg(feature = "napi")]
const MIN_RAYON_THREADS: usize = 1;

// Production: Use OnceLock directly for lock-free access after initialization
// Test: Keep RwLock variant for shutdown_global_pool() functionality
#[cfg(all(not(test), feature = "napi"))]
pub(crate) static GLOBAL_THREAD_POOL: OnceLock<std::result::Result<Arc<ThreadPool>, String>> =
    OnceLock::new();

#[cfg(all(test, feature = "napi"))]
pub(crate) static GLOBAL_THREAD_POOL: OnceLock<
    RwLock<Option<std::result::Result<Arc<ThreadPool>, String>>>,
> = OnceLock::new();

#[cfg(all(test, feature = "napi"))]
fn pool_cell() -> &'static RwLock<Option<std::result::Result<Arc<ThreadPool>, String>>> {
    GLOBAL_THREAD_POOL.get_or_init(|| RwLock::new(None))
}

#[cfg(feature = "napi")]
fn build_pool() -> std::result::Result<Arc<ThreadPool>, String> {
    let detected_parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(MIN_RAYON_THREADS);

    let uv_reserve = reserved_libuv_threads();
    let num_threads = detected_parallelism
        .saturating_sub(uv_reserve)
        .max(MIN_RAYON_THREADS);

    match rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
    {
        Ok(pool) => Ok(Arc::new(pool)),
        Err(primary_err) => rayon::ThreadPoolBuilder::new()
            .num_threads(MIN_RAYON_THREADS)
            .build()
            .map(Arc::new)
            .map_err(|fallback_err| {
                format!(
                    "preferred pool ({num_threads} threads) failed: {primary_err}; \
                     fallback pool ({MIN_RAYON_THREADS} thread) failed: {fallback_err}"
                )
            }),
    }
}

#[cfg(feature = "napi")]
fn pool_init_error(message: &str) -> LazyImageError {
    LazyImageError::internal_panic(format!("failed to initialize rayon thread pool: {message}"))
}

// Production: Lock-free access via OnceLock::get_or_init()
#[cfg(all(not(test), feature = "napi"))]
pub fn get_pool() -> std::result::Result<Arc<ThreadPool>, LazyImageError> {
    match GLOBAL_THREAD_POOL.get_or_init(build_pool) {
        Ok(pool) => Ok(Arc::clone(pool)),
        Err(message) => Err(pool_init_error(message)),
    }
}

// Test: Keep double-check locking for shutdown_global_pool() compatibility
#[cfg(all(test, feature = "napi"))]
pub fn get_pool() -> std::result::Result<Arc<ThreadPool>, LazyImageError> {
    {
        let guard = pool_cell().read();
        if let Some(pool) = guard.as_ref() {
            return pool
                .as_ref()
                .map(Arc::clone)
                .map_err(|message| pool_init_error(message));
        }
    }

    let mut guard = pool_cell().write();
    if let Some(pool) = guard.as_ref() {
        return pool
            .as_ref()
            .map(Arc::clone)
            .map_err(|message| pool_init_error(message));
    }

    let pool = build_pool();
    *guard = Some(pool);
    guard
        .as_ref()
        .expect("pool cell was initialized")
        .as_ref()
        .map(Arc::clone)
        .map_err(|message| pool_init_error(message))
}

/// Explicitly drop the global thread pool so it can be re-created.
/// This is primarily used in tests and controlled lifecycles (e.g., module reload).
#[cfg(all(test, feature = "napi"))]
pub(crate) fn shutdown_global_pool() {
    if let Some(pool) = pool_cell().write().take() {
        drop(pool);
    }
}

/// Drop and immediately reinitialize the global thread pool.
/// Useful for scenarios where environment variables (like UV_THREADPOOL_SIZE)
/// change at runtime and need to be respected by a fresh pool instance.
#[cfg(all(test, feature = "napi"))]
pub(crate) fn reinitialize_global_pool() -> std::result::Result<Arc<ThreadPool>, LazyImageError> {
    shutdown_global_pool();
    get_pool()
}

/// Calculates optimal concurrency based on CPU and memory constraints
///
/// This function combines CPU-based parallelism detection with memory-aware
/// concurrency limits to prevent OOM kills in constrained containers.
///
/// # Returns
/// Optimal concurrency value (number of concurrent operations)
#[cfg(feature = "napi")]
pub fn calculate_optimal_concurrency() -> usize {
    // 1. Detect CPU-based parallelism
    let cpu_based = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(MIN_RAYON_THREADS);

    // Reserve threads for libuv
    let uv_reserve = reserved_libuv_threads();
    let cpu_concurrency = cpu_based.saturating_sub(uv_reserve).max(MIN_RAYON_THREADS);

    // 2. Detect memory limits and calculate memory-based concurrency
    let available_memory = memory::detect_available_memory();
    let memory_based =
        memory::calculate_memory_based_concurrency(available_memory, cpu_concurrency);

    // 3. Use the minimum of CPU and memory constraints
    // This ensures we don't exceed either CPU or memory limits
    memory_based
}

#[cfg(feature = "napi")]
pub fn resolve_effective_concurrency(requested_concurrency: u32) -> usize {
    if requested_concurrency == 0 {
        calculate_optimal_concurrency()
    } else {
        requested_concurrency as usize
    }
}

#[cfg(feature = "napi")]
fn reserved_libuv_threads() -> usize {
    std::env::var("UV_THREADPOOL_SIZE")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIBUV_THREADPOOL_SIZE)
}

#[cfg(all(test, feature = "napi"))]
mod tests {
    use super::*;
    use image::imageops::FilterType;
    use image::{DynamicImage, ImageBuffer, Rgb};
    use rayon::prelude::*;
    use std::io::Cursor;

    struct EnvGuard {
        original: Option<String>,
        key: &'static str,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { original, key }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.original.as_ref() {
                Some(val) => std::env::set_var(self.key, val),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn expected_threads(uv_size: usize) -> usize {
        let detected = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(MIN_RAYON_THREADS);
        detected.saturating_sub(uv_size).max(MIN_RAYON_THREADS)
    }

    fn thread_count(pool: &Arc<ThreadPool>) -> usize {
        pool.install(rayon::current_num_threads)
    }

    fn make_workload() -> Vec<DynamicImage> {
        (0..6)
            .map(|i| {
                let width = 64 + i * 8;
                let height = 48 + i * 6;
                let buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                    ImageBuffer::from_fn(width, height, |x, y| {
                        Rgb([((x + y) % 255) as u8, (x % 255) as u8, (y % 255) as u8])
                    });
                DynamicImage::ImageRgb8(buffer)
            })
            .collect()
    }

    #[test]
    fn pool_reinitializes_with_new_uv_reservation() {
        let guard = EnvGuard::set("UV_THREADPOOL_SIZE", "8");

        let pool = reinitialize_global_pool().expect("pool should initialize");
        let expected = expected_threads(8);
        assert_eq!(thread_count(&pool), expected);

        drop(guard);
        let pool_after_reset = reinitialize_global_pool().expect("pool should reinitialize");
        let expected_default = expected_threads(DEFAULT_LIBUV_THREADPOOL_SIZE);
        assert_eq!(thread_count(&pool_after_reset), expected_default);
    }

    #[test]
    fn resolve_effective_concurrency_prefers_manual_value() {
        assert_eq!(resolve_effective_concurrency(7), 7);
    }

    #[test]
    fn pool_handles_real_workloads_and_stays_usable() {
        shutdown_global_pool();
        let pool = get_pool().expect("pool should initialize");
        let images = make_workload();

        let resized: Vec<Vec<u8>> = pool.install(|| {
            images
                .par_iter()
                .map(|img| {
                    let resized = img.resize(96, 72, FilterType::Triangle);
                    let mut buf = Vec::new();
                    resized
                        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
                        .expect("encode should succeed");
                    buf
                })
                .collect()
        });

        assert!(resized.iter().all(|buf| !buf.is_empty()));

        let squares: Vec<u32> = pool.install(|| {
            (0..128u32)
                .into_par_iter()
                .map(|n| n.saturating_mul(n))
                .collect()
        });
        assert_eq!(squares.len(), 128);
    }
}
