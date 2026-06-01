export type WasmImageInput = File | Blob | ArrayBuffer | Uint8Array;
export type WasmOutputFormat = 'jpeg' | 'webp';
export type WasmOutputKind = 'blob' | 'arrayBuffer' | 'uint8Array';
export type WasmResizeFit = 'inside' | 'cover' | 'fill';
export type WasmQualityFloorPolicy = 'best-effort' | 'strict';
export type WasmProfile = 'upload-safe' | 'avatar' | 'attachment' | 'balanced';
export type WasmRuntime = 'browser' | 'browser-worker' | 'edge-isolate';

export interface CreateUploadOptimizerOptions {
  wasmModules?: {
    jpegDecode?: WebAssembly.Module | ArrayBuffer | Uint8Array;
    jpegEncode?: WebAssembly.Module | ArrayBuffer | Uint8Array;
    pngDecode?: WebAssembly.Module | ArrayBuffer | Uint8Array;
    resize?: WebAssembly.Module | ArrayBuffer | Uint8Array;
    webpDecode?: WebAssembly.Module | ArrayBuffer | Uint8Array;
    webpEncode?: WebAssembly.Module | ArrayBuffer | Uint8Array;
  };
  defaultOutput?: WasmOutputKind;
  runtime?: WasmRuntime;
  now?: () => number;
}

export interface WasmOptimizeOptions {
  format: WasmOutputFormat;
  output?: WasmOutputKind;
  maxWidth?: number;
  maxHeight?: number;
  fit?: WasmResizeFit;
  targetBytes?: number;
  minQuality?: number;
  maxQuality?: number;
  qualityFloorPolicy?: WasmQualityFloorPolicy;
  stripMetadata?: boolean;
  profile?: WasmProfile;
  limits?: WasmLimits;
  signal?: AbortSignal;
}

export interface WasmLimits {
  maxBytes?: number;
  maxPixels?: number;
  timeoutMs?: number;
}

export interface WasmOptimizeResult {
  data: Blob | ArrayBuffer | Uint8Array;
  format: WasmOutputFormat;
  qualityUsed: number;
  bytesOut: number;
  budgetMet: boolean;
  targetBytes?: number;
  metrics: WasmProcessingMetrics;
}

export interface WasmProcessingMetrics {
  version: string;
  runtime: WasmRuntime;
  bytesIn: number;
  bytesOut: number;
  formatIn?: string | null;
  formatOut: WasmOutputFormat;
  widthIn?: number;
  heightIn?: number;
  widthOut?: number;
  heightOut?: number;
  qualityUsed: number;
  budgetMet: boolean;
  targetBytes?: number;
  instantiateMs?: number;
  decodeMs: number;
  opsMs: number;
  encodeMs: number;
  totalMs: number;
  firstEncodeMs?: number;
  metadataStripped: boolean;
  policyViolations: string[];
  memory?: {
    peakBytes?: number;
    source: 'performance-api' | 'runtime-estimate' | 'unavailable';
  };
}

export declare class LazyImageWasmError extends Error {
  code: string;
  category: 'Input' | 'Processing' | 'Output' | 'Config' | 'Internal';
  recoverable: boolean;
  recoveryHint?: string;
}

export declare const VERSION: string;
