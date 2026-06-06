import type { CreateUploadOptimizerOptions, WasmImageInput, WasmOptimizeOptions, WasmOptimizeResult } from './shared.js';

export declare function createUploadOptimizer(
  options?: CreateUploadOptimizerOptions
): Promise<{
  optimizeUpload(input: WasmImageInput, options: WasmOptimizeOptions): Promise<WasmOptimizeResult>;
}>;

export declare function optimizeUpload(
  input: WasmImageInput,
  options: WasmOptimizeOptions
): Promise<WasmOptimizeResult>;
