'use strict';

// Deterministic chronology extraction; GC overlap is not a causal attribution.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const read = file => JSON.parse(fs.readFileSync(file));
const root = process.argv[2];
assert.ok(root, 'Usage: analyze.cjs <completed measurement directory>');
assert.ok(fs.existsSync(path.join(root, 'completed.json')), 'Incomplete batch');
const intervals = [], timings = [], allocations = [];
for (const dir of fs.readdirSync(root).sort()) {
  if (!fs.statSync(path.join(root, dir)).isDirectory()) continue;
  const sample = read(path.join(root, dir, 'sample.json'));
  assert.equal(sample.status, 'PASS');
  const trace = read(path.join(root, dir, 'trace.json'));
  const [, arm, pair, side] = dir.match(/^(.+)-(\d+)-([ab])$/);
  const log = fs.readFileSync(path.join(root, dir + '.log'), 'utf8');
  const reported = [...log.matchAll(/(?:^| )allocated=(\d+)/gm)].map(m => Number(m[1]));
  allocations.push({ arm, pair: Number(pair), side, eventsWithAllocatedField: reported.length,
    sumOfReportedAllocatedFields: reported.length ? reported.reduce((a, b) => a + b, 0) : null,
    scope: 'Sum of V8 trace NVP allocated fields across this process; not exact pipeline allocation or retained bytes. Passive runs have no allocation trace.' });
  sample.samples.forEach(s => {
    for (const [name, values] of Object.entries(s.timings)) for (const [engine, ms] of Object.entries(values)) timings.push({ arm, pair: Number(pair), side, phase: s.phase, name, engine, ms });
  });
  const calls = trace.intervals.filter(i => i.name.startsWith('native:'));
  const count = sample.isolated ? 1 : 4;
  calls.forEach((call, index) => {
    const before = call.before, after = call.after;
    const gc = trace.gc.filter(e => e.startMs < after.atMs && e.startMs + e.durationMs > before.atMs);
    const overlap = gc.reduce((n, e) => n + Math.max(0, Math.min(after.atMs, e.startMs + e.durationMs) - Math.max(before.atMs, e.startMs)), 0);
    intervals.push({ arm, pair: Number(pair), side, phase: sample.samples[Math.floor(index / count)].phase,
      name: sample.isolated || index % count === 3 ? 'Complex Pipeline' : ['JPEG', 'WebP', 'AVIF'][index % count],
      startMs: before.atMs, endMs: after.atMs, wallMs: after.atMs - before.atMs,
      processCpuMs: (after.cpu.user + after.cpu.system - before.cpu.user - before.cpu.system) / 1000,
      voluntarySwitches: after.usage.voluntaryContextSwitches - before.usage.voluntaryContextSwitches,
      involuntarySwitches: after.usage.involuntaryContextSwitches - before.usage.involuntaryContextSwitches,
      minorFaults: after.usage.minorPageFault - before.usage.minorPageFault,
      majorFaults: after.usage.majorPageFault - before.usage.majorPageFault,
      memoryBefore: before.memory, memoryAfter: after.memory,
      eventLoopActiveMs: after.elu.active - before.elu.active,
      gcEventCount: gc.length, gcDurationOverlapMs: overlap, gcEvents: gc,
      ...(trace.nativeTiming ? { native: trace.nativeTiming[index] } : {}) });
  });
}
const median = values => { const s = [...values].sort((a, b) => a - b); return (s[Math.floor((s.length - 1) / 2)] + s[Math.floor(s.length / 2)]) / 2; };
const orderEffects = [];
for (const arm of [...new Set(timings.map(r => r.arm))]) for (const phase of ['first', 'warm']) for (const order of ['AB', 'BA']) {
  const rows = timings.filter(r => r.arm === arm && r.name === 'Complex Pipeline' && r.engine === 'lazy' && (phase === 'first' ? r.phase === 'first' : r.phase.startsWith('warm')) && (r.pair % 2 ? 'AB' : 'BA') === order);
  const ratios = [...new Set(rows.map(r => r.pair))].map(pair => {
    const values = side => rows.filter(r => r.pair === pair && r.side === side).map(r => r.ms);
    return median(values('b')) / median(values('a'));
  });
  assert.ok(ratios.length > 0);
  orderEffects.push({ arm, phase, order, pairs: ratios.length, ratios, medianRatio: median(ratios) });
}
const result = { limits: ['CPU and context switches aggregate the entire process, including background GC/native threads.', 'GC duration overlap is an observed interval intersection, not time uniquely caused by GC or a precise stop-the-world duration.', 'Event-loop utilization describes JS-thread activity, not worker queue duration.', 'Native total excludes memory admission and JS resolve; wall remainder is not precise queue waiting.', 'No per-core affinity, core residency, clock frequency or original historical host trace is available.'], orderEffects, allocations, intervals };
fs.writeFileSync(path.join(root, 'chronology.json'), JSON.stringify(result, null, 2) + '\n');
const keys = ['arm', 'pair', 'side', 'phase', 'name', 'engine', 'ms'];
fs.writeFileSync(path.join(root, 'raw-timings.csv'), keys.join(',') + '\n' + timings.map(row => keys.map(k => row[k]).join(',')).join('\n') + '\n');
console.log(JSON.stringify({ processes: new Set(timings.map(r => `${r.arm}-${r.pair}-${r.side}`)).size, timedOutputs: timings.length, observedNativeCalls: intervals.length,
  firstComplex: intervals.filter(i => i.phase === 'first' && i.name === 'Complex Pipeline').map(i => ({ arm: i.arm, pair: i.pair, side: i.side, wallMs: i.wallMs, processCpuMs: i.processCpuMs, gcOverlapMs: i.gcDurationOverlapMs, heapBefore: i.memoryBefore.heapUsed, native: i.native })) }));
