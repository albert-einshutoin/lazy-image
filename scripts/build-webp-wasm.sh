#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "$0")/.." && pwd)
docker run --rm --platform linux/amd64 \
  -v "$repo_root/packages/lazy-image-wasm/vendor/webp:/codec" \
  emscripten/emsdk:3.1.73@sha256:a2abf158f65e3024da9883bac1d20bfc4ed936539293edff5469ca20b4a5dc90 \
  bash -euo pipefail -c '
    work=$(mktemp -d)
    cd "$work"
    curl -fsSL https://codeload.github.com/webmproject/libwebp/tar.gz/refs/tags/v1.6.0 -o libwebp.tar.gz
    echo "93a852c2b3efafee3723efd4636de855b46f9fe1efddd607e1f42f60fc8f2136  libwebp.tar.gz" | sha256sum -c -
    tar -xzf libwebp.tar.gz
    # libwebp 1.6.0 stops the Emscripten probes before SSE2, although its
    # SSE4.1 implementation uses shared SSE2 helpers. Include that probe.
    sed -i "s/GREATER_EQUAL 2)/GREATER_EQUAL 3)/" libwebp-1.6.0/cmake/cpu.cmake
    for variant in baseline simd; do
      simd=OFF
      flags=""
      if [ "$variant" = simd ]; then simd=ON; flags="-msimd128 -msse2"; fi
      emcmake cmake -S libwebp-1.6.0 -B "$variant" \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_FLAGS="$flags" \
        -DWEBP_ENABLE_SIMD="$simd" -DWEBP_USE_THREAD=OFF \
        -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF \
        -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF \
        -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF
      cmake --build "$variant" -j 4
      codecs="enc/webp_enc"
      if [ "$variant" = baseline ]; then codecs="$codecs dec/webp_dec"; fi
      for codec in $codecs; do
        output="$codec"
        if [ "$variant" = simd ]; then output="${codec}_simd"; fi
        em++ -O3 $flags --bind -I libwebp-1.6.0 \
          /codec/codec/"$codec.cpp" "$variant/libwebp.a" "$variant/libsharpyuv.a" \
          --pre-js /codec/codec/pre.js -sALLOW_MEMORY_GROWTH=1 \
          -sEXPORT_ES6=1 -sENVIRONMENT=web,worker -sDYNAMIC_EXECUTION=0 \
          -sMODULARIZE=1 -sFILESYSTEM=0 -o /codec/codec/"$output.js"
      done
    done
    cp libwebp-1.6.0/COPYING /codec/codec/LICENSE.codec.md
  '
