# Lazy Semantics

In `lazy-image`, "lazy" does not mean "constructors do nothing."

## What is eager

- `ImageEngine.from(buffer)` copies the input buffer into Rust-owned memory.
- `ImageEngine.fromPath(path)` synchronously validates and opens the source. Files at or below 256 MB are fully read into Rust-owned memory on the calling thread; larger files use mmap setup with an advisory lock and retained file descriptor.

## What is deferred

- Full pixel decode.
- Auto-orientation and all queued image operations.
- Output encoding (`toBuffer`, `toFile`, metrics variants, preset variants).
- `ImageEngine.fromPathAsync(path)` source setup runs on an N-API worker; after its Promise resolves, decode and pipeline execution remain deferred like the synchronous API.
- ICC and EXIF extraction is lazy and cached on first use.

## Contract

- `lazy-image` is a **lazy decode / lazy execution** pipeline, not a zero-work constructor API.
- `fromPath()` is synchronous compatibility behavior and can block the event loop while reading up to 256 MB. Prefer `await fromPathAsync()` in request-serving code.
- Both path APIs preserve the same SIGBUS policy: in-memory reads through 256 MB, then mmap with advisory lock/fd retention (or a memory fallback when the lock cannot be acquired).
- If you need metadata only, use `inspect()` / `inspectFile()` instead of building a pipeline.
