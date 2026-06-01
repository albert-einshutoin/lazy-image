import { createUploadOptimizer } from './browser.js';

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
        error: {
          name: error.name,
          message: error.message,
          code: error.code,
          category: error.category,
          recoverable: error.recoverable,
          recoveryHint: error.recoveryHint,
        },
      });
    }
  };
}
