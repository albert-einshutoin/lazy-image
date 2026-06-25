// When copying this file into your own Worker project, change the next two
// imports to the package specifiers '@alberteinshutoin/lazy-image-wasm/edge'
// and '@alberteinshutoin/lazy-image-wasm/shared'.
import { createUploadOptimizer } from '../edge.js';
import { LazyImageWasmError } from '../shared.js';

let optimizerPromise;

function resolveWasmModules(env) {
  const modules = {
    jpegDecode: env?.JSQUASH_JPEG_DECODER,
    jpegEncode: env?.JSQUASH_JPEG_ENCODER,
    pngDecode: env?.JSQUASH_PNG_DECODER,
    resize: env?.JSQUASH_RESIZE,
    webpDecode: env?.JSQUASH_WEBP_DECODER,
    webpEncode: env?.JSQUASH_WEBP_ENCODER,
  };
  return Object.fromEntries(
    Object.entries(modules).filter(([, value]) => value !== undefined)
  );
}

export function getEdgeUploadOptimizer(env = {}) {
  if (!optimizerPromise) {
    // Wasm bindings are static per deployment, so the first request's env wires
    // the optimizer for the lifetime of the isolate. Reset on failure so one
    // bad init does not cache a rejected promise until isolate eviction.
    optimizerPromise = createUploadOptimizer({
      wasmModules: resolveWasmModules(env),
      defaultOutput: 'arrayBuffer',
      now: () => Date.now(),
    }).catch((error) => {
      optimizerPromise = undefined;
      throw error;
    });
  }

  return optimizerPromise;
}

export async function handleOptimizeRequest(request, env = {}) {
  if (request.method !== 'POST') {
    return new Response('POST a binary image body to optimize', {
      status: 405,
      headers: { allow: 'POST' },
    });
  }

  const data = new Uint8Array(await request.arrayBuffer());
  if (data.byteLength === 0) {
    return new Response('Request body is empty', { status: 400 });
  }

  try {
    const optimizer = await getEdgeUploadOptimizer(env);
    const result = await optimizer.optimizeUpload(data, {
      format: 'jpeg',
      maxWidth: 1600,
      maxHeight: 1600,
    });

    return new Response(result.data, {
      headers: {
        'content-type': 'image/jpeg',
        'x-lazy-image-metric-bytes-out': String(result.metrics.bytesOut),
        'x-lazy-image-metric-encode-ms': String(result.metrics.encodeMs),
      },
    });
  } catch (error) {
    if (error instanceof LazyImageWasmError) {
      const isCallerError = error.category === 'Input' || error.category === 'Config';
      return new Response(`[${error.code}] ${error.message}`, {
        status: isCallerError ? 400 : 500,
      });
    }
    throw error;
  }
}

export default {
  async fetch(request, env) {
    return handleOptimizeRequest(request, env);
  },
};
