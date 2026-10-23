# WebP codec provenance

These codecs use libwebp **1.6.0**, including the CVE-2023-4863 fix.
They replace the libwebp 1.1.0 Wasm embedded in `@jsquash/webp 1.5.0`.
Both baseline and SIMD encoders and the decoder are rebuilt; there is no
runtime path to the old npm codec.
The shared loader also rejects explicit bindings reporting libwebp older
than 1.3.2 or no version; the decoder initializer awaits that validation.

The JavaScript API and C++ bindings come from jSquash commit
`8bcd212da8c2be7c9c223e8f222eb3d9574a713b`, package `@jsquash/webp 1.5.0`.
Its Apache-2.0 license is retained in `LICENSE`.
The libwebp BSD license is retained in `codec/LICENSE.codec.md`.

Rebuild from the repository root with `npm run build:webp-wasm` (Docker required).
`scripts/build-webp-wasm.sh` pins the Emscripten 3.1.73 image digest and verifies
the libwebp source archive SHA-256 before building. Its source archive is
`https://codeload.github.com/webmproject/libwebp/tar.gz/refs/tags/v1.6.0` with
SHA-256 `93a852c2b3efafee3723efd4636de855b46f9fe1efddd607e1f42f60fc8f2136`.
The build recipe includes the SSE2 compiler probe skipped by upstream's
Emscripten probe loop, because its SSE4.1 code depends on shared SSE2 helpers.
The package tests check the version reported by every generated codec,
accept valid WebP through browser/Edge/Worker entrypoints, and reject truncated input.

Keep the existing asset names when copying the Wasm alongside a bundled Worker.
Replace this vendor when a reviewed upstream package ships a patched codec.
