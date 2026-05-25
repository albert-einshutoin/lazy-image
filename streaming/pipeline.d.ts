import type { OutputFormat, ProcessingMetrics, ResizeFit } from "../index";

export interface StreamingOperation {
  op: "resize" | "rotate" | "flipH" | "flipV" | "grayscale" | "autoOrient";
  width?: number;
  height?: number;
  fit?: ResizeFit;
  degrees?: number;
  enabled?: boolean;
}

export interface CreateStreamingPipelineOptions {
  format?: OutputFormat;
  quality?: number;
  ops?: StreamingOperation[];
  ImageEngine?: typeof import("../index").ImageEngine;
  onMetrics?: (metrics: ProcessingMetrics) => void;
  onCleanupError?: (err: Error, target: string) => void;
}

export declare function createStreamingPipeline(
  options?: CreateStreamingPipelineOptions
): {
  writable: NodeJS.WritableStream;
  readable: NodeJS.ReadableStream;
};
