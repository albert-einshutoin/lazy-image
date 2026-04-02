import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const mod = require('./index.js');

// Re-export all named exports from the CJS module
export const {
  ImageEngine,
  ErrorCategory,
  ErrorCode,
  inspect,
  inspectFile,
  supportedInputFormats,
  supportedOutputFormats,
  version,
  getErrorCategory,
  createStreamingPipeline,
  resolveEncodeProfile,
} = mod;

// Default export for convenience
export default mod;
