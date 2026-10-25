// When copying this file into your own Worker project, change the next two
// imports to the package specifiers '@alberteinshutoin/lazy-image-wasm/edge'
// and '@alberteinshutoin/lazy-image-wasm/shared'.
import { createUploadOptimizer } from '../edge.js';
import { DEFAULT_LIMITS, LazyImageWasmError } from '../shared.js';

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

export async function handleOptimizeRequest(request, env = {}, authorize) {
  if (request.method !== 'POST') {
    return new Response('POST a binary image body to optimize', {
      status: 405,
      headers: { allow: 'POST' },
    });
  }

  // The application supplies its authentication/admission decision. Refuse to
  // turn the copied example into an anonymous public codec endpoint by default.
  if (typeof authorize !== 'function') return new Response('Upload authorization is not configured', { status: 503 });
  if (await authorize(request) !== true) return new Response('Unauthorized', { status: 401 });
  try {
    const length = request.headers.get('content-length');
    if (length !== null && (!/^\d+$/.test(length) || !Number.isSafeInteger(Number(length)))) {
      return new Response('Invalid Content-Length', { status: 400 });
    }
    if (length !== null && Number(length) > DEFAULT_LIMITS.maxBytes) return new Response('Request body exceeds 32 MiB', { status: 413 });
    if (!request.body) return new Response('Request body is empty', { status: 400 });
    const reader = request.body.getReader();
    const chunks = [];
    let size = 0;
    try {
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        size += value.byteLength;
        if (size > DEFAULT_LIMITS.maxBytes) {
          await reader.cancel();
          return new Response('Request body exceeds 32 MiB', { status: 413 });
        }
        chunks.push(value);
      }
    } finally {
      reader.releaseLock();
    }
    if (!size) return new Response('Request body is empty', { status: 400 });
    const data = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) { data.set(chunk, offset); offset += chunk.byteLength; }
    const optimizer = await getEdgeUploadOptimizer(env);
    const result = await optimizer.optimizeUpload(data, {
      format: 'jpeg',
      maxWidth: 1600,
      maxHeight: 1600,
      limits: DEFAULT_LIMITS,
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
      // E111/E131 are CodecError in the shared native taxonomy, but at this
      // HTTP boundary they still describe caller-supplied input. Keep encoder
      // CodecErrors such as E300 as server failures.
      const isCallerError =
        error.category === 'UserError' || error.code === 'E111' || error.code === 'E131';
      return new Response(`[${error.code}] ${error.message}`, {
        status: error.category === 'ResourceLimit' ? 413 : isCallerError ? 400 : 500,
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
