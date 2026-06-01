import type { CreateUploadOptimizerOptions } from './shared.js';

export declare function createUploadWorkerHandler(options?: CreateUploadOptimizerOptions): (event: MessageEvent) => Promise<void>;
