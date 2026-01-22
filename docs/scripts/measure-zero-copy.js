/**
 * Zero-copy RSS/heap 測定スクリプト
 * 実行: node --expose-gc docs/scripts/measure-zero-copy.js
 */

const path = require('path');
const fs = require('fs');
const { ImageEngine } = require('../..');

const SOURCE = path.resolve(__dirname, '../../test/fixtures/test_4.5MB_5000x5000.png');

function mb(bytes) {
  return Math.round((bytes / 1024 / 1024) * 10) / 10;
}

async function main() {
  if (!fs.existsSync(SOURCE)) {
    console.error('Fixture not found:', SOURCE);
    process.exit(1);
  }

  if (typeof global.gc === 'function') {
    global.gc();
  }
  const rssStart = process.memoryUsage().rss;
  const heapStart = process.memoryUsage().heapUsed;

  const { metrics } = await ImageEngine.fromPath(SOURCE)
    .resize(2000, 2000, 'inside')
    .toBufferWithMetrics('png');

  if (typeof global.gc === 'function') {
    global.gc();
  }
  const rssEnd = process.memoryUsage().rss;
  const heapEnd = process.memoryUsage().heapUsed;

  const result = {
    source: path.basename(SOURCE),
    rss_start_mb: mb(rssStart),
    rss_end_mb: mb(rssEnd),
    rss_delta_mb: mb(rssEnd - rssStart),
    heap_delta_mb: mb(heapEnd - heapStart),
    peak_rss_metrics_mb: mb(metrics.peakRss || 0),
  };

  console.log(JSON.stringify(result, null, 2));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
