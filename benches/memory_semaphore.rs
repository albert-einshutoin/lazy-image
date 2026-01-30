use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use parking_lot::{Condvar as PlCondvar, Mutex as PlMutex};
use std::sync::{Arc, Condvar as StdCondvar, Mutex as StdMutex};
use std::thread;

// Baseline: previous std::sync implementation
#[derive(Debug)]
struct StdWeightedSemaphore {
    capacity: u64,
    state: StdMutex<u64>,
    cvar: StdCondvar,
}

impl StdWeightedSemaphore {
    fn new(capacity: u64) -> Self {
        Self {
            capacity,
            state: StdMutex::new(capacity),
            cvar: StdCondvar::new(),
        }
    }

    fn acquire(&self, weight: u64) {
        let mut available = self.state.lock().unwrap();
        let need = weight.min(self.capacity);
        while *available < need {
            available = self.cvar.wait(available).unwrap();
        }
        *available -= need;
    }

    fn release(&self, weight: u64) {
        let mut available = self.state.lock().unwrap();
        let freed = (*available).saturating_add(weight).min(self.capacity);
        *available = freed;
        self.cvar.notify_all();
    }
}

// Current parking_lot implementation (mirrors production)
#[derive(Debug)]
struct PlWeightedSemaphore {
    capacity: u64,
    state: PlMutex<u64>,
    cvar: PlCondvar,
}

impl PlWeightedSemaphore {
    fn new(capacity: u64) -> Self {
        Self {
            capacity,
            state: PlMutex::new(capacity),
            cvar: PlCondvar::new(),
        }
    }

    fn acquire(&self, weight: u64) {
        let mut available = self.state.lock();
        let need = weight.min(self.capacity);
        while *available < need {
            self.cvar.wait(&mut available);
        }
        *available -= need;
    }

    fn release(&self, weight: u64) {
        let mut available = self.state.lock();
        let freed = (*available).saturating_add(weight).min(self.capacity);
        *available = freed;
        self.cvar.notify_all();
    }
}

fn hammer_semaphore_std(iterations: usize, threads: usize) {
    let sem = Arc::new(StdWeightedSemaphore::new(threads as u64));
    let mut handles = Vec::with_capacity(threads);
    for _ in 0..threads {
        let sem = Arc::clone(&sem);
        handles.push(thread::spawn(move || {
            for _ in 0..iterations {
                sem.acquire(1);
                sem.release(1);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}

fn hammer_semaphore_pl(iterations: usize, threads: usize) {
    let sem = Arc::new(PlWeightedSemaphore::new(threads as u64));
    let mut handles = Vec::with_capacity(threads);
    for _ in 0..threads {
        let sem = Arc::clone(&sem);
        handles.push(thread::spawn(move || {
            for _ in 0..iterations {
                sem.acquire(1);
                sem.release(1);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}

fn bench_contention(c: &mut Criterion) {
    // 64 threads × 2_000 iterations = 128k acquire/release pairs
    let iterations = 2_000;
    let threads = 64;

    c.bench_function("std_mutex_semaphore", |b| {
        b.iter_batched(
            || (),
            |_| hammer_semaphore_std(iterations, threads),
            BatchSize::SmallInput,
        )
    });

    c.bench_function("parking_lot_semaphore", |b| {
        b.iter_batched(
            || (),
            |_| hammer_semaphore_pl(iterations, threads),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(memory_semaphore, bench_contention);
criterion_main!(memory_semaphore);
