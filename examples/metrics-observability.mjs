/**
 * Metrics observability example.
 *
 * This converts ProcessingMetrics into structured logs and an in-process
 * aggregate that can feed Prometheus, OpenTelemetry, Datadog, or another
 * monitoring backend.
 *
 * Run:
 *   node examples/metrics-observability.mjs <imagePath>
 *   node examples/metrics-observability.mjs --self-test
 */

import assert from 'node:assert/strict';
import { fixturePath, hasSelfTestFlag, loadLazyImage } from './_load.mjs';

const { ImageEngine } = loadLazyImage();

if (hasSelfTestFlag()) {
  await runSelfTest();
} else {
  const input = process.argv[2] || 'input.jpg';
  await runDemo(input);
}

function createAggregate() {
  return {
    processed: 0,
    bytesIn: 0,
    bytesOut: 0,
    totalMs: [],
  };
}

function recordMetrics(aggregate, metrics) {
  aggregate.processed += 1;
  aggregate.bytesIn += metrics.bytesIn;
  aggregate.bytesOut += metrics.bytesOut;
  aggregate.totalMs.push(metrics.totalMs);

  return summarizeAggregate(aggregate);
}

function summarizeAggregate(aggregate) {
  return {
    processed: aggregate.processed,
    bytesIn: aggregate.bytesIn,
    bytesOut: aggregate.bytesOut,
    p50TotalMs: percentile(aggregate.totalMs, 0.5),
    p95TotalMs: percentile(aggregate.totalMs, 0.95),
  };
}

function percentile(values, p) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1);
  return sorted[index];
}

function structuredLog(metrics, extra = {}) {
  return {
    event: 'image_optimized',
    format_in: metrics.formatIn,
    format_out: metrics.formatOut,
    bytes_in: metrics.bytesIn,
    bytes_out: metrics.bytesOut,
    compression_ratio: metrics.compressionRatio,
    duration_ms: metrics.totalMs,
    decode_ms: metrics.decodeMs,
    encode_ms: metrics.encodeMs,
    ops_ms: metrics.opsMs,
    icc_preserved: metrics.iccPreserved,
    metadata_stripped: metrics.metadataStripped,
    ...extra,
  };
}

async function optimizeAndRecord(input, aggregate, index = 0) {
  const { data, metrics } = await ImageEngine.fromPath(input)
    .resize({ width: 640 + index * 8, fit: 'inside' })
    .toBufferWithMetrics('webp', 80);

  const log = structuredLog(metrics, { output_bytes: data.length });
  console.log(JSON.stringify(log));

  // Monitoring field mapping:
  // decode_ms / encode_ms / ops_ms / duration_ms -> histogram
  // compression_ratio -> gauge
  // bytes_in / bytes_out / output_bytes -> counter or distribution
  // format_in / format_out -> low-cardinality labels
  return recordMetrics(aggregate, metrics);
}

async function runDemo(input) {
  const aggregate = createAggregate();
  const summary = await optimizeAndRecord(input, aggregate);
  console.log(JSON.stringify({ event: 'lazy_image_metrics_summary', ...summary }));
}

async function runSelfTest() {
  const aggregate = createAggregate();
  const input = fixturePath('test_100KB_1057x1057.jpg');
  let last = summarizeAggregate(aggregate);

  for (let i = 0; i < 3; i += 1) {
    const next = await optimizeAndRecord(input, aggregate, i);
    assert(next.processed > last.processed);
    assert(next.bytesIn >= last.bytesIn);
    assert(next.bytesOut >= last.bytesOut);
    assert(next.p95TotalMs >= 0);
    last = next;
  }

  assert.equal(last.processed, 3);
  assert(last.bytesIn > 0);
  assert(last.bytesOut > 0);
}
