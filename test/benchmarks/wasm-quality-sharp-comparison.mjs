import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';
import { makeReference, readChecked, score, sha256, ssimOptions } from './wasm-quality-metrics.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const target = path.join(root, 'docs/history/v1.4.0/sharp-0.35.4-comparison');
const original = path.join(root, 'docs/history/v1.4.0/quality-evaluation/quality-results.json');
const sourceFiles = [
  'test/benchmarks/wasm-quality-sharp-comparison.mjs',
  'test/benchmarks/wasm-quality-metrics.mjs',
];
const relative = (file) => path.relative(root, file);

async function pixels(bytes) {
  const { data, info } = await sharp(bytes).toColourspace('srgb').ensureAlpha()
    .raw({ depth: 'uchar' }).toBuffer({ resolveWithObject: true });
  return { data, width: info.width, height: info.height, sha256: sha256(data) };
}

function pixelDifference(before, after) {
  assert.equal(before.width, after.width);
  assert.equal(before.height, after.height);
  assert.equal(before.data.length, after.data.length);
  let changedChannels = 0;
  let sumAbsoluteDifference = 0;
  let maxAbsoluteDifference = 0;
  for (let i = 0; i < before.data.length; i++) {
    const difference = Math.abs(before.data[i] - after.data[i]);
    if (difference) changedChannels++;
    sumAbsoluteDifference += difference;
    maxAbsoluteDifference = Math.max(maxAbsoluteDifference, difference);
  }
  return { changedChannels, totalChannels: before.data.length,
    meanAbsoluteDifference: sumAbsoluteDifference / before.data.length,
    maxAbsoluteDifference };
}

function metricDelta(before, after, name) {
  assert.equal(before.status, after.status, `${name} status changed`);
  return before.status === 'measured' ? after.value - before.value : 0;
}

async function sourceEvidence() {
  const codeSha = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
  const dirty = execFileSync('git', ['status', '--porcelain', '--', ...sourceFiles],
    { cwd: root, encoding: 'utf8' }).trim();
  assert.equal(dirty, '', 'analysis code must be committed before analysis');
  const hashes = Object.fromEntries(await Promise.all(sourceFiles.map(async (file) =>
    [file, sha256(await fs.readFile(path.join(root, file)))])));
  return { codeSha, hashes };
}

async function baseline(report, source) {
  assert.equal(sharp.versions.sharp, '0.35.0');
  const references = {};
  for (const [id, item] of Object.entries(report.references)) {
    const input = await readChecked(path.join(root, item.input.path), item.input.sha256);
    const oldBytes = await readChecked(path.join(root, item.reference.path), item.reference.sha256);
    const generated = await makeReference(input, item.policyDimensions);
    assert.equal(sha256(generated.bytes), item.reference.sha256,
      `old reference is not reproducible: ${id}`);
    const oldPixels = await pixels(oldBytes);
    references[id] = { input: item.input, policyDimensions: item.policyDimensions,
      reference: item.reference, preResizeOrientedSrgbSha256: generated.orientedSrgbSha256,
      pixelSha256: oldPixels.sha256, width: oldPixels.width, height: oldPixels.height };
  }
  const cases = {};
  for (const item of report.cases) {
    const reference = await readChecked(path.join(root, item.reference.path), item.reference.sha256);
    const output = await readChecked(path.join(root, item.output.path), item.output.sha256);
    const outputPixels = await pixels(output);
    const quality = await score(reference, output);
    assert.equal(quality.status, 'measured');
    cases[item.id] = { inputId: item.inputId, output: item.output,
      outputPixelSha256: outputPixels.sha256, quality,
      oldReportSsimDelta: metricDelta(item.quality.ssim, quality.ssim, item.id),
      oldReportPsnrDeltaDb: metricDelta(item.quality.psnr, quality.psnr, item.id) };
  }
  return { analyzedAt: new Date().toISOString(), analysisCodeSha: source.codeSha,
    analysisSourceHashes: source.hashes, originalReport: relative(original),
    originalReportSha256: sha256(await fs.readFile(original)),
    originalAnalysisCodeSha: report.analysisCodeSha,
    tool: { sharpVersions: sharp.versions, ssim: { version: '3.5.0', options: ssimOptions } },
    references, cases };
}

async function updated(report, source, old) {
  assert.equal(sharp.versions.sharp, '0.35.4');
  assert.equal(old.analysisCodeSha, source.codeSha, 'baseline uses different analysis code');
  assert.deepEqual(old.analysisSourceHashes, source.hashes);
  assert.equal(old.originalReportSha256, sha256(await fs.readFile(original)));
  const references = {};
  for (const [id, item] of Object.entries(report.references)) {
    const input = await readChecked(path.join(root, item.input.path), item.input.sha256);
    const oldBytes = await readChecked(path.join(root, item.reference.path), item.reference.sha256);
    const generated = await makeReference(input, item.policyDimensions);
    const newFile = path.join(target, `reference-${id}.png`);
    await fs.writeFile(newFile, generated.bytes);
    const before = await pixels(oldBytes);
    const after = await pixels(generated.bytes);
    references[id] = { input: item.input, policyDimensions: item.policyDimensions,
      oldReference: item.reference, newReference: { path: relative(newFile),
        sha256: sha256(generated.bytes), bytes: generated.bytes.length },
      oldPixelSha256UnderOldSharp: old.references[id].pixelSha256,
      oldPixelSha256UnderNewSharp: before.sha256,
      newPixelSha256UnderNewSharp: after.sha256,
      oldReferenceDecodeChanged: before.sha256 !== old.references[id].pixelSha256,
      pngBytesChanged: sha256(generated.bytes) !== item.reference.sha256,
      referencePixelsChanged: before.sha256 !== after.sha256,
      referencePixelDifference: pixelDifference(before, after),
      oldPreResizeOrientedSrgbSha256: item.preResizeOrientedSrgbSha256,
      newPreResizeOrientedSrgbSha256: generated.orientedSrgbSha256 };
  }
  const cases = {};
  for (const item of report.cases) {
    const oldRef = await readChecked(path.join(root, item.reference.path), item.reference.sha256);
    const newRef = await readChecked(path.join(root, references[item.inputId].newReference.path),
      references[item.inputId].newReference.sha256);
    const output = await readChecked(path.join(root, item.output.path), item.output.sha256);
    const outputPixels = await pixels(output);
    const oldReferenceQuality = await score(oldRef, output);
    const newReferenceQuality = await score(newRef, output);
    assert.equal(oldReferenceQuality.status, 'measured');
    assert.equal(newReferenceQuality.status, 'measured');
    cases[item.id] = { inputId: item.inputId, output: item.output,
      outputPixelSha256UnderOldSharp: old.cases[item.id].outputPixelSha256,
      outputPixelSha256UnderNewSharp: outputPixels.sha256,
      outputDecodeChanged: outputPixels.sha256 !== old.cases[item.id].outputPixelSha256,
      oldReferenceQuality, newReferenceQuality,
      decodeAndScoringDelta: {
        ssim: metricDelta(old.cases[item.id].quality.ssim, oldReferenceQuality.ssim, item.id),
        psnrDb: metricDelta(old.cases[item.id].quality.psnr, oldReferenceQuality.psnr, item.id) },
      referenceGenerationDelta: {
        ssim: metricDelta(oldReferenceQuality.ssim, newReferenceQuality.ssim, item.id),
        psnrDb: metricDelta(oldReferenceQuality.psnr, newReferenceQuality.psnr, item.id) } };
  }
  return { analyzedAt: new Date().toISOString(), analysisCodeSha: source.codeSha,
    analysisSourceHashes: source.hashes, baseline: 'baseline.json',
    originalReport: old.originalReport, originalReportSha256: old.originalReportSha256,
    originalAnalysisCodeSha: old.originalAnalysisCodeSha,
    oldTool: old.tool, newTool: { sharpVersions: sharp.versions,
      ssim: { version: '3.5.0', options: ssimOptions } },
    comparisons: { oldSavedReferenceAndOutput: 'decode and score only',
      originalInputToNewReference: 'same autoOrient, sRGB, inside Lanczos3, dimensions and PNG conditions' },
    references, cases };
}

function markdown(result) {
  const refs = Object.entries(result.references);
  const cases = Object.entries(result.cases);
  const max = (key) => Math.max(...cases.map(([, item]) => Math.abs(item[key].ssim)));
  const maxPsnr = (key) => Math.max(...cases.map(([, item]) => Math.abs(item[key].psnrDb)));
  const lines = [
    '# sharp 0.35.4 による保存済み Wasm 品質証拠の比較', '',
    `解析日時: ${result.analyzedAt} / 解析コードSHA: \`${result.analysisCodeSha}\`。`,
    `旧記録: [quality-results.json](../quality-evaluation/quality-results.json)（SHA-256 \`${result.originalReportSha256}\`、当時の解析コード \`${result.originalAnalysisCodeSha}\`）。`,
    '今回の [baseline.json](baseline.json) と [comparison.json](comparison.json) は別ディレクトリに保存。旧参照・raw・出力・結果は変更していない。',
    '', `旧: sharp ${result.oldTool.sharpVersions.sharp} / libvips ${result.oldTool.sharpVersions.vips} / libheif ${result.oldTool.sharpVersions.heif}。`,
    `新: sharp ${result.newTool.sharpVersions.sharp} / libvips ${result.newTool.sharpVersions.vips} / libheif ${result.newTool.sharpVersions.heif}。`,
    '採点: ssim.js 3.5.0、元記録と同一のsRGB 8-bit RGB PSNR・SSIM条件。入力、旧参照、保存済み出力はSHA-256検証済み。',
    '', '## 旧参照・保存済み出力のdecodeと採点', '',
    `16ケース中、出力画素hashが変わったもの: ${cases.filter(([, x]) => x.outputDecodeChanged).length}。`,
    `旧参照画素hashが変わったもの: ${refs.filter(([, x]) => x.oldReferenceDecodeChanged).length}。`,
    `旧参照を固定したSSIM差の最大絶対値: ${max('decodeAndScoringDelta')}、PSNR差: ${maxPsnr('decodeAndScoringDelta')} dB。`,
    '', '## 元入力からの参照再生成', '',
    '| 入力 | PNG bytes hash変化 | デコード後の参照画素変化 | 変更チャネル数 / 全チャネル | 最大絶対差 |',
    '|---|---|---|---:|---:|',
    ...refs.map(([id, x]) => `| ${id} | ${x.pngBytesChanged} | ${x.referencePixelsChanged} | ${x.referencePixelDifference.changedChannels} / ${x.referencePixelDifference.totalChannels} | ${x.referencePixelDifference.maxAbsoluteDifference} |`),
    '', `新参照で採点したSSIM差の最大絶対値: ${max('referenceGenerationDelta')}、PSNR差: ${maxPsnr('referenceGenerationDelta')} dB。`,
    'ケース別のhash、画素digest、両軸のSSIM/PSNR値と差はcomparison.jsonに記録。',
    '', '保存済みWasm出力bytesが同一なので、この再解析差はWasmの画質回帰を示さない。用途別の画質十分性は対象用途・表示条件・実容量・権利・判定者が揃うまで未判定。',
  ];
  return `${lines.join('\n')}\n`;
}

async function main() {
  const phase = process.argv[2];
  assert(['baseline', 'updated'].includes(phase), 'usage: node wasm-quality-sharp-comparison.mjs baseline|updated');
  const report = JSON.parse(await fs.readFile(original, 'utf8'));
  assert.equal(report.cases.length, 16);
  const source = await sourceEvidence();
  await fs.mkdir(target, { recursive: true });
  if (phase === 'baseline') {
    const result = await baseline(report, source);
    await fs.writeFile(path.join(target, 'baseline.json'), `${JSON.stringify(result, null, 2)}\n`);
    console.log('Saved baseline for 5 references and 16 outputs with sharp 0.35.0');
  } else {
    const old = JSON.parse(await fs.readFile(path.join(target, 'baseline.json'), 'utf8'));
    const result = await updated(report, source, old);
    await fs.writeFile(path.join(target, 'comparison.json'), `${JSON.stringify(result, null, 2)}\n`);
    await fs.writeFile(path.join(target, 'RESULTS.md'), markdown(result));
    console.log('Saved separate comparison for 5 references and 16 outputs with sharp 0.35.4');
  }
}

main().catch((error) => { console.error(error.stack || error.message); process.exitCode = 1; });
