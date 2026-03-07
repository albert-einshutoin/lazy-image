# Lazy Semantics

In `lazy-image`, "lazy" does not mean "constructors do nothing."

## What is eager

- `ImageEngine.from(buffer)` copies the input buffer into Rust-owned memory.
- `ImageEngine.from(buffer)` and `ImageEngine.fromPath(path)` extract lightweight metadata needed for later decisions, including ICC and EXIF payloads when present.
- `ImageEngine.fromPath(path)` validates the path and opens the source using the safe file-loading path (in-memory read for smaller files, mmap for large files).

## What is deferred

- Full pixel decode.
- Auto-orientation and all queued image operations.
- Output encoding (`toBuffer`, `toFile`, metrics variants, preset variants).

## Contract

- `lazy-image` is a **lazy decode / lazy execution** pipeline, not a zero-work constructor API.
- Constructor work is intentionally limited to cheap source setup and metadata extraction.
- If you need metadata only, use `inspect()` / `inspectFile()` instead of building a pipeline.
