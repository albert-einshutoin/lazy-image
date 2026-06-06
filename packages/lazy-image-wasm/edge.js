import { createUploadOptimizer as createBrowserUploadOptimizer } from './browser.js';

export async function createUploadOptimizer(options = {}) {
  return createBrowserUploadOptimizer({
    ...options,
    defaultOutput: options.defaultOutput ?? 'arrayBuffer',
    runtime: 'edge-isolate',
  });
}

export async function optimizeUpload(input, options) {
  const optimizer = await createUploadOptimizer();
  return optimizer.optimizeUpload(input, options);
}
