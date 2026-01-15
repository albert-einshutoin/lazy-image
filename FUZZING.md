# Fuzzing lazy-image

This repository ships a [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) setup under
`fuzz/` to stress critical entry points such as the decoder pipeline, encoder pipeline,
ICC profile parsing, and header inspection helpers. The goal is to ensure corrupted or
adversarial inputs never trigger panics or memory-safety bugs.

## Requirements

- Rust nightly toolchain
- [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) (`cargo install cargo-fuzz`)
- macOS/Linux host (libFuzzer requires UNIX signals)
- cmake and nasm (for libavif-sys)

## Available fuzz targets

| Target | Description | APIs Tested |
| --- | --- | --- |
| `decode_from_buffer` | Tests lazy-image's decoders with arbitrary bytes | `decode_jpeg_mozjpeg`, `decode_with_image_crate`, `inspect_header_from_bytes` |
| `encode_to_format` | Tests encoding to all supported formats | `encode_jpeg`, `encode_png`, `encode_webp`, `encode_avif` |
| `pipeline_ops` | Tests image operations pipeline | `apply_ops` (resize, crop, rotate, flip, brightness, contrast) |
| `inspect_header` | Critical attack surface – header-only metadata parsing | `inspect_header_from_bytes` |
| `icc_profile` | Tests ICC profile extraction from various containers | `extract_icc_profile` (JPEG, PNG, WebP, AVIF) |

Seed corpora live in `fuzz/seeds/` (tiny.jpg, tiny.png, tiny.webp) and can be
expanded with additional minimal samples for better coverage.

## Running locally

```bash
# Run a specific target
cargo +nightly fuzz run decode_from_buffer

# Run with time limit (recommended)
cargo +nightly fuzz run decode_from_buffer -- -max_total_time=60

# Run all targets for 1 minute each
for target in decode_from_buffer encode_to_format pipeline_ops inspect_header icc_profile; do
  cargo +nightly fuzz run $target -- -max_total_time=60
done
```

Notes:
- `cargo fuzz` automatically builds the `lazy-image-fuzz` crate located in `fuzz/`.
- The harness links against the `lazy-image` library with the `fuzzing` feature
  to expose internal helpers without pulling in N-API bindings.

### AddressSanitizer

To run with ASAN locally for enhanced memory error detection:

```bash
RUSTFLAGS="-Zsanitizer=address" \
  cargo +nightly fuzz run inspect_header -- -max_total_time=60
```

### Memory limits

For CI/resource-constrained environments, use memory limits:

```bash
cargo +nightly fuzz run encode_to_format -- \
  -max_len=1048576 \
  -rss_limit_mb=2048 \
  -max_total_time=300
```

## CI Integration

Fuzzing runs automatically via GitHub Actions (`.github/workflows/fuzz.yml`):

- **Schedule**: Nightly at 3:00 UTC
- **Duration**: 5 minutes per target
- **Memory limit**: 2GB RSS
- **Crash handling**: Auto-creates GitHub issues with `bug`, `security`, `fuzz-crash` labels

### Manual trigger

You can manually trigger fuzzing from GitHub Actions:
1. Go to Actions → Fuzz workflow
2. Click "Run workflow"
3. Optionally specify duration (seconds) or specific target

## Corpus management

- Corpora are cached between CI runs for incremental coverage improvement
- New interesting inputs are automatically saved to `fuzz/corpus/<target>/`
- Crash reproducers are exported to `fuzz/artifacts/<target>/`

To minimize a crash input:

```bash
cargo +nightly fuzz tmin <target> <crash_file>
```

## Reporting issues

If fuzzing uncovers a panic or memory-safety problem:

1. Check `fuzz/artifacts/<target>/` for the crashing input
2. Minimize it with `cargo fuzz tmin`
3. File an issue with the minimized reproducer
4. Mention the fuzz target that triggered it

For security-critical crashes, please use responsible disclosure and contact
the maintainers privately before public disclosure.
