import { createUploadOptimizer } from './browser.js';
import { LazyImageWasmError } from './shared.js';

function workerError(error) {
  let name = 'Error';
  let message = 'Wasm upload optimization failed; cause unavailable.';
  try {
    if (typeof error?.name === 'string') name = error.name;
    if (typeof error?.message === 'string') message = error.message;
    if (error instanceof LazyImageWasmError) {
      return { name, message, code: error.code, category: error.category,
        recoverable: error.recoverable, recoveryHint: error.recoveryHint };
    }
  } catch {
    return { name, message };
  }
  return { name, message };
}

export function createUploadWorkerHandler(options = {}) {
  const optimizerPromise = createUploadOptimizer({
    ...options,
    runtime: 'browser-worker',
  });

  return async function handleUploadOptimizationMessage(event) {
    const { id, input, options: optimizeOptions } = event.data ?? {};
    try {
      const optimizer = await optimizerPromise;
      const result = await optimizer.optimizeUpload(input, optimizeOptions);
      event.target?.postMessage?.({ id, ok: true, result });
    } catch (error) {
      event.target?.postMessage?.({
        id,
        ok: false,
        error: workerError(error),
      });
    }
  };
}
