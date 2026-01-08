# Fuzzing lazy-image

This repository ships a [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) setup under
`fuzz/` to stress critical entry points such as the decoder pipeline and header
inspection helpers. The goal is to ensure corrupted or adversarial inputs never
trigger panics or memory-safety bugs.

## Requirements

- Rust nightly toolchain
- [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) (`cargo install cargo-fuzz`)
- macOS/Linux host (libFuzzer requires UNIX signals)

## Available fuzz targets

| Target | Description |
| --- | --- |
| `decode_from_buffer` | Feeds arbitrary bytes through `EncodeTask::decode` to hit JPEG (mozjpeg) and `image` decoders. |
| `pipeline_ops` | Applies randomly generated resize/crop/rotate operations to decoded images, ensuring lazy pipeline steps never panic. |
| `inspect_header` | Critical attack surface – fuzzes `inspect_header_from_bytes` used by the JS `inspect()`/`inspectFile()` APIs. |

Seed corpora for header inspection live in `fuzz/corpus/inspect_header/` (1×1 PNG/JPEG)
and can be expanded with additional minimal samples for better coverage.

## Running locally

```bash
rustup run nightly cargo fuzz run decode_from_buffer
rustup run nightly cargo fuzz run pipeline_ops
rustup run nightly cargo fuzz run inspect_header
```

Notes:
- `cargo fuzz` automatically builds the `lazy-image-fuzz` crate located in `fuzz/`.
- The harness links against the `lazy-image` library with the lightweight `fuzzing`
  feature to expose inspect helpers without pulling in N-API bindings.

### AddressSanitizer

To run with ASAN locally:

```bash
rustup run nightly RUSTFLAGS="-Zsanitizer=address" \
  cargo fuzz run inspect_header -- -max_total_time=60
```

## CI / Automation

Fuzzing is meant to run on a scheduled job (e.g. nightly or weekly) so that long
libFuzzer sessions can accumulate coverage. Integrate via:

```bash
rustup run nightly cargo fuzz run inspect_header -- -max_total_time=600
```

Example GitHub Actions workflow:

```yaml
name: fuzz
on:
  schedule:
    - cron: '0 3 * * 0'
jobs:
  inspect_header:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - run: |
          RUSTFLAGS="-Zsanitizer=address" \
          cargo fuzz run inspect_header -- -max_total_time=600
```

Store resulting corpora in `fuzz/corpus/<target>/` if new interesting inputs are
found, and export crash reproducers into `fuzz/artifacts/<target>/` for triage.

## Reporting issues

If fuzzing uncovers a panic or memory-safety problem, please file an issue
with the crashing input (or minimized reproducer) and mention the fuzz target
that triggered it.
