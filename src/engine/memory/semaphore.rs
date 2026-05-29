// src/engine/memory/semaphore.rs
//
// Fair, weighted in-process semaphore used to throttle concurrent image
// operations by their estimated peak memory footprint.

use parking_lot::{Condvar, Mutex};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Waiter entry in the fair queue.
/// Stores the required weight and a flag to signal readiness.
#[derive(Debug)]
struct Waiter {
    weight: u64,
    ready: Arc<AtomicBool>,
}

/// State protected by mutex.
#[derive(Debug)]
struct SemaphoreState {
    available: u64,
    waiters: VecDeque<Waiter>,
}

/// In-memory weighted semaphore for byte-based backpressure.
/// Uses a fair FIFO queue and wake-all handoff for correctness.
#[derive(Debug)]
pub struct WeightedSemaphore {
    capacity: u64,
    state: Mutex<SemaphoreState>,
    cvar: Condvar,
}

#[derive(Debug)]
pub struct MemoryPermit {
    sem: Arc<WeightedSemaphore>,
    weight: u64,
}

impl MemoryPermit {
    /// Effective weight reserved by this permit (after any capacity clamping).
    #[allow(dead_code)] // exposed for test/observability paths
    pub(super) fn weight(&self) -> u64 {
        self.weight
    }
}

impl WeightedSemaphore {
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            state: Mutex::new(SemaphoreState {
                available: capacity,
                waiters: VecDeque::new(),
            }),
            cvar: Condvar::new(),
        }
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Returns `true` if the last `acquire()` was clamped because the requested
    /// weight exceeded the semaphore capacity. This is purely informational;
    /// the permit already serializes the oversized operation by reserving full
    /// capacity.
    #[allow(dead_code)] // Public API, tested, not yet called from production code paths
    pub fn was_clamped(permit: &MemoryPermit, original_weight: u64) -> bool {
        original_weight > permit.weight
    }

    pub fn acquire(self: &Arc<Self>, weight: u64) -> MemoryPermit {
        if weight == 0 {
            return MemoryPermit {
                sem: Arc::clone(self),
                weight: 0,
            };
        }

        // If the request exceeds capacity, acquire FULL capacity instead.
        // This serializes oversized operations (only one can run at a time)
        // rather than silently under-reserving. The caller can check whether
        // clamping occurred via `WeightedSemaphore::was_clamped()`.
        let need = if weight > self.capacity {
            #[cfg(debug_assertions)]
            eprintln!(
                "lazy-image: memory request ({} bytes) exceeds semaphore capacity ({} bytes); \
                 acquiring full capacity to serialize this operation",
                weight, self.capacity
            );
            self.capacity
        } else {
            weight
        };

        let ready_flag = {
            let mut state = self.state.lock();

            // Fast path: acquire immediately only when no older waiter is queued.
            // Once a waiter exists, every new request must join the queue to
            // preserve FIFO fairness and avoid starving large operations.
            if state.waiters.is_empty() && state.available >= need {
                state.available -= need;
                return MemoryPermit {
                    sem: Arc::clone(self),
                    weight: need,
                };
            }

            // Slow path: enqueue and wait
            let ready = Arc::new(AtomicBool::new(false));
            state.waiters.push_back(Waiter {
                weight: need,
                ready: Arc::clone(&ready),
            });
            ready
        };

        // Wait for our turn (FIFO fairness)
        loop {
            if ready_flag.load(Ordering::Acquire) {
                return MemoryPermit {
                    sem: Arc::clone(self),
                    weight: need,
                };
            }

            // Sleep briefly before checking again
            let mut guard = self.state.lock();
            if !ready_flag.load(Ordering::Acquire) {
                self.cvar.wait(&mut guard);
            }
        }
    }

    fn release(&self, weight: u64) {
        let mut state = self.state.lock();

        // Return capacity
        state.available = state.available.saturating_add(weight).min(self.capacity);

        let mut woke_any = false;
        // Fair wake: satisfy waiters in FIFO order.
        while let Some(waiter) = state.waiters.front() {
            if state.available >= waiter.weight {
                let waiter = state.waiters.pop_front().unwrap();
                state.available -= waiter.weight;
                woke_any = true;
                waiter.ready.store(true, Ordering::Release);
            } else {
                break; // Can't satisfy next waiter, stop
            }
        }

        if woke_any || (!state.waiters.is_empty() && state.available > 0) {
            self.cvar.notify_all();
        }
    }

    /// Snapshot of currently-available capacity. Test-only; not part of the
    /// public API because the value races with concurrent acquire/release.
    #[cfg(test)]
    pub(super) fn available_for_test(&self) -> u64 {
        self.state.lock().available
    }

    #[cfg(test)]
    pub(super) fn waiters_for_test(&self) -> usize {
        self.state.lock().waiters.len()
    }
}

impl Drop for MemoryPermit {
    fn drop(&mut self) {
        self.sem.release(self.weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn acquire_release_returns_capacity() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        let permit = sem.acquire(60);
        assert_eq!(sem.available_for_test(), 40);
        drop(permit);
        assert_eq!(sem.available_for_test(), 100);
    }

    #[test]
    fn zero_weight_acquire_returns_immediately() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        let permit = sem.acquire(0);
        assert_eq!(permit.weight, 0);
        // Available capacity should be unchanged
        assert_eq!(sem.available_for_test(), 100);
    }

    #[test]
    fn oversized_request_acquires_full_capacity() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        // Request more than capacity
        let permit = sem.acquire(200);
        // Should have clamped to full capacity (100), not the requested 200
        assert_eq!(permit.weight, 100);
        assert_eq!(sem.available_for_test(), 0);
        // was_clamped should report true
        assert!(WeightedSemaphore::was_clamped(&permit, 200));
        drop(permit);
        assert_eq!(sem.available_for_test(), 100);
    }

    #[test]
    fn normal_request_not_clamped() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        let permit = sem.acquire(60);
        assert_eq!(permit.weight, 60);
        assert!(!WeightedSemaphore::was_clamped(&permit, 60));
    }

    #[test]
    fn wakes_waiter_after_drop() {
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
        rx_started
            .recv_timeout(Duration::from_secs(1))
            .expect("waiter should signal start");
        drop(permit); // release full capacity

        rx_done
            .recv_timeout(Duration::from_secs(1))
            .expect("waiter should acquire after release");
        handle.join().unwrap();
    }

    #[test]
    fn queued_waiter_blocks_later_fast_path() {
        let sem = Arc::new(WeightedSemaphore::new(100));
        let permit = sem.acquire(80);

        let sem_large = Arc::clone(&sem);
        let (large_acquired_tx, large_acquired_rx) = std::sync::mpsc::channel();
        let large = thread::spawn(move || {
            let _permit = sem_large.acquire(100);
            large_acquired_tx.send(()).unwrap();
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while sem.waiters_for_test() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "large waiter should enqueue"
            );
            thread::yield_now();
        }

        let sem_small = Arc::clone(&sem);
        let (small_acquired_tx, small_acquired_rx) = std::sync::mpsc::channel();
        let small = thread::spawn(move || {
            let _permit = sem_small.acquire(10);
            small_acquired_tx.send(()).unwrap();
        });

        assert!(
            small_acquired_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "later small waiter must not bypass an older queued waiter"
        );

        drop(permit);
        large_acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("large waiter should acquire first after release");
        small_acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("small waiter should acquire after large permit drops");

        large.join().unwrap();
        small.join().unwrap();
    }

    #[test]
    fn oversized_requests_are_serialized() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let sem = Arc::new(WeightedSemaphore::new(100));
        let concurrent_count = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..4 {
            let sem = Arc::clone(&sem);
            let concurrent = Arc::clone(&concurrent_count);
            let max_conc = Arc::clone(&max_concurrent);
            handles.push(thread::spawn(move || {
                // Each requests more than capacity → must serialize
                let _permit = sem.acquire(200);
                let prev = concurrent.fetch_add(1, Ordering::SeqCst);
                // Update max concurrent
                let current = prev + 1;
                max_conc.fetch_max(current, Ordering::SeqCst);
                // Small sleep to give other threads a chance to contend
                thread::sleep(Duration::from_millis(10));
                concurrent.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // At most 1 oversized request should run at a time since each
        // acquires full capacity
        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            1,
            "oversized requests should be serialized (max 1 concurrent)"
        );
    }
}
