import { createRequire } from 'module';
const require = createRequire(import.meta.url);
export const { createStreamingPipeline } = require('./pipeline.js');
