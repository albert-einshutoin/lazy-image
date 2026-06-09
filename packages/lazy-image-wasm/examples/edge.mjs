import { createUploadOptimizer } from '../edge.js';

let optimizerPromise;

function resolveWasmModules(env) {
  return {
    jpegDecode: env?.JPEGSQUASH_JPEG_DECODER,
    jpegEncode: env?.JPEGSQUASH_JPEG_ENCODER,
    pngDecode: env?.JPEGSQUASH_PNG_DECODER,
    resize: env?.JPEGSQUASH_RESIZE,
    webpDecode: env?.JPEGSQUASH_WEBP_DECODER,
    webpEncode: env?.JPEGSQUASH_WEBP_ENCODER,
  };
}

export async function getEdgeUploadOptimizer(env = {}) {
  const wasmModules = resolveWasmModules(env);
  const filteredModules = Object.fromEntries(
    Object.entries(wasmModules).filter(([, value]) => value !== undefined)
  );

  if (!optimizerPromise) {
    optimizerPromise = createUploadOptimizer({
      wasmModules: filteredModules,
      defaultOutput: 'arrayBuffer',
      now: () => Date.now(),
    });
  }

  return optimizerPromise;
}

export async function handleOptimizeRequest(request, env = {}) {
  const optimizer = await getEdgeUploadOptimizer(env);
  const data = new Uint8Array(await request.arrayBuffer());
  const result = await optimizer.optimizeUpload(data, {
    format: 'jpeg',
    maxWidth: 1600,
    maxHeight: 1600,
    output: 'arrayBuffer',
  });

  return new Response(result.data, {
    headers: {
      'content-type': 'image/jpeg',
      'x-lazy-image-metric-bytes-out': String(result.metrics.bytesOut),
      'x-lazy-image-metric-encode-ms': String(result.metrics.encodeMs),
    },
  });
}

export default {
  async fetch(request, env) {
    if (request.method !== 'POST' || !request.body) {
      return new Response('POST a binary image body to optimize', { status: 400 });
    }

    return handleOptimizeRequest(request, env);
  },
};

