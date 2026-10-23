# libavif security backports

Base: crates.io `libavif-sys 0.17.0+libavif.1.0.4`, archive SHA-256
`490ca74b61e773140bebb086c81facb23273a99364a9ab0c92ab1532b90ade35`.
The crate is vendored because its current published release still embeds
libavif 1.0.4. Its ABI and rav1e 0.7.1 encoder are retained.
The root dependency is path-only so Cargo publication cannot silently
replace these backports with the unpatched registry crate. Publishing a Rust
crate requires a separately reviewed dependency/publication decision.

Local changes:

- CVE-2025-48174: reject `offset + size` and allocation-growth overflow in
  `libavif/src/stream.c`. Based on the upstream [overflow checks](https://github.com/AOMediaCodec/libavif/commit/e5fdefe7d1776e6c4cf1703c163a8c0535599029)
  and the complete guards in [libavif 1.3.0](https://github.com/AOMediaCodec/libavif/blob/v1.3.0/src/stream.c).
- CVE-2025-48175: promote RGB and YUV strides to `size_t` before multiplying
  row indexes in `avifImageRGBToYUV`. Backport of
  [64d956ed5a602f78cebf29da023280944ee92efd](https://github.com/AOMediaCodec/libavif/commit/64d956ed5a602f78cebf29da023280944ee92efd).

The upstream BSD license is retained in `libavif/LICENSE`.
Replace this vendor with a reviewed upstream release containing both fixes
when one is available. Recheck `paste` separately; these C backports do not
resolve its informational maintenance warning.
