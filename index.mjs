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
  compilerIdentity,
  getErrorCategory,
  processBatchChunked,
  createStreamingPipeline,
  resolveEncodeProfile,
  compileArtifactPlan,
  compileImage,
} = mod;

// Default export for convenience
export default mod;
