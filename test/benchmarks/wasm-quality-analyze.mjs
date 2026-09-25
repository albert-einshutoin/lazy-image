import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';
import { makeReference, pixelMetadata, readChecked, score, sha256, ssimOptions } from './wasm-quality-metrics.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const target = path.join(root, 'docs/history/v1.4.0/quality-evaluation');
const history = path.join(root, 'docs/history/v1.4.0');
const original = [
  { runtime: 'node-wasm', dir: 'node-wasm', key: 'nodeResults' },
  { runtime: 'chrome-worker-default', dir: 'wasm-published', key: 'browserResults' },
  { runtime: 'local-workerd', dir: 'edge-workerd', key: 'edgeResults' },
];
const savedInputs = {
  'jpeg-webp': 'test/fixtures/test_3.2MB_5000x5000.jpg',
  'png-jpeg': 'test/fixtures/test_4.5MB_5000x5000.png',
  'metadata-budget': 'docs/history/v1.4.0/node-wasm/wasm-metadata-input.jpg',
};
const savedOrder = ['jpeg-webp', 'png-jpeg', 'metadata-budget', 'metadata-budget-best-effort'];
const crops = {
  'jpeg-webp': { left: 672, top: 672, width: 256, height: 256 },
  'png-jpeg': { left: 672, top: 672, width: 256, height: 256 },
  'metadata-budget': { left: 42, top: 72, width: 96, height: 96 },
  coffee: { left: 96, top: 55, width: 96, height: 96 },
  chelsea: { left: 112, top: 48, width: 96, height: 96 },
};
const relative = (file) => path.relative(root, file);
const shaFile = async (file) => sha256(await fs.readFile(file));
const sourceFiles = ['test/benchmarks/wasm-quality-analyze.mjs',
  'test/benchmarks/wasm-quality-metrics.mjs'];

async function reference(id, inputFile, expectedHash, dimensions, references) {
  const inputBytes = await readChecked(inputFile, expectedHash);
  const generated = await makeReference(inputBytes, dimensions);
  const file = path.join(target, `reference-${id}.png`);
  await fs.writeFile(file, generated.bytes);
  const record = { input: { path: relative(inputFile), sha256: expectedHash,
    bytes: inputBytes.length, ...generated.input },
    reference: { path: relative(file), sha256: sha256(generated.bytes),
      bytes: generated.bytes.length, ...generated.reference },
    preResizeOrientedSrgbSha256: generated.orientedSrgbSha256,
    policyDimensions: dimensions,
    referenceTool: { sharp: sharp.versions.sharp, libvips: sharp.versions.vips,
      jpeg: sharp.versions.mozjpeg, png: sharp.versions.png, webp: sharp.versions.webp,
      lcms: sharp.versions.lcms } };
  references[id] = record;
  return generated.bytes;
}

function assertSame(a, b, label) {
  assert.deepEqual(a, b, `${label} differs across runtime evidence`);
}

async function savedCases(references) {
  const runs = await Promise.all(original.map(async (item) => {
    const rawFile = path.join(history, item.dir, 'positive-evidence.json');
    const evidence = JSON.parse(await fs.readFile(rawFile, 'utf8'));
    assert.equal(evidence.verdict, 'PASS');
    assert.equal(evidence.publishedVersion, '1.4.0');
    const results = item.key === 'nodeResults' ? evidence.nodeResults : evidence[item.key].results;
    assert.equal(results.length, 3);
    return { ...item, rawFile, rawSha256: await shaFile(rawFile), evidence, results };
  }));
  const cases = [];
  for (const id of savedOrder) {
    const sourceId = id.replace('-best-effort', '');
    const linked = [];
    for (const run of runs) {
      const fixture = run.evidence.fixtures.find((entry) => entry.id === sourceId);
      const result = run.results.find((entry) => entry.id === sourceId);
      assert(fixture && result && result.status === 'PASS');
      const bestEffort = id.endsWith('-best-effort');
      const stored = bestEffort ? result.budget?.bestEffort : result;
      assert(stored?.output, `missing output evidence: ${run.runtime}/${id}`);
      const file = path.join(history, run.dir, path.basename(stored.output.artifact));
      const bytes = await readChecked(file, stored.output.sha256);
      const metadata = await pixelMetadata(bytes);
      assert.equal(bytes.length, stored.output.bytes);
      assert.equal(metadata.format, stored.output.format);
      assert.equal(metadata.width, stored.output.width);
      assert.equal(metadata.height, stored.output.height);
      assert.equal(bestEffort ? stored.budgetMet : result.metrics.budgetMet,
        bestEffort ? false : bytes.length <= fixture.options.targetBytes);
      const inputFile = sourceId === 'metadata-budget'
        ? path.join(history, run.dir, 'wasm-metadata-input.jpg')
        : path.join(root, savedInputs[sourceId]);
      await readChecked(inputFile, fixture.inputSha256);
      assert.equal(fixture.inputBytes, (await fs.stat(inputFile)).size);
      linked.push({ runtime: run.runtime, rawEvidence: relative(run.rawFile),
        rawEvidenceSha256: run.rawSha256, generatedAt: run.evidence.generatedAt,
        imageGenerationCodeSha: run.evidence.sourceSha, input: relative(inputFile),
        inputSha256: fixture.inputSha256, policy: bestEffort
          ? { ...fixture.options, targetBytes: 10, minQuality: 50, maxQuality: 51,
            qualityFloorPolicy: 'best-effort' } : fixture.options,
        output: relative(file), outputSha256: stored.output.sha256,
        bytes: bytes.length, width: metadata.width, height: metadata.height,
        format: metadata.format, targetBytes: bestEffort ? 10 : fixture.options.targetBytes,
        budgetMet: bestEffort ? stored.budgetMet : result.metrics.budgetMet,
        qualityUsed: bestEffort ? null : result.metrics.qualityUsed,
        qualityNote: bestEffort ? 'original best-effort raw evidence omitted selected quality' : null });
    }
    const first = linked[0];
    for (const item of linked.slice(1)) {
      for (const key of ['inputSha256', 'policy', 'outputSha256', 'bytes', 'width', 'height',
        'format', 'targetBytes', 'budgetMet', 'qualityUsed']) {
        assertSame(item[key], first[key], `${id}/${key}`);
      }
    }
    const dimensions = sourceId === 'metadata-budget' ? { width: 320, height: 240 }
      : { width: 1600, height: 1600 };
    const referenceId = sourceId;
    let refBytes;
    if (!references[referenceId]) refBytes = await reference(referenceId,
      path.join(root, savedInputs[sourceId]), first.inputSha256, dimensions, references);
    else refBytes = await readChecked(path.join(root, references[referenceId].reference.path),
      references[referenceId].reference.sha256);
    const outputBytes = await readChecked(path.join(root, first.output), first.outputSha256);
    const quality = await score(refBytes, outputBytes);
    cases.push({ id, inputId: referenceId, category: 'synthetic',
      imageGeneration: linked, sharedScoring: true,
      input: references[referenceId].input, reference: references[referenceId].reference,
      output: { path: first.output, sha256: first.outputSha256, bytes: first.bytes,
        width: first.width, height: first.height, format: first.format },
      format: first.format, targetBytes: first.targetBytes, budgetMet: first.budgetMet,
      qualityUsed: first.qualityUsed, qualityNote: first.qualityNote, quality });
  }
  return cases;
}

async function photoCases(references) {
  const file = path.join(target, 'photo-run-evidence.json');
  const run = JSON.parse(await fs.readFile(file, 'utf8'));
  assert.equal(run.verdict, 'PASS');
  assert.equal(run.package.tarballIntegrity,
    'sha512-R3xpMJr3rBv+LsyfpggixCZvktlS3XRvLi542x4JgbrvvBP3NH/xip3TJSze4fGDQIfj7hWplLeKUlHPCnidew==');
  assert.equal(run.cases.length, 12);
  const rawHash = await shaFile(file);
  const manifest = JSON.parse(await fs.readFile(path.join(root, 'test/benchmarks/corpus/release-manifest.json'), 'utf8'));
  const refBytes = {};
  for (const id of ['coffee', 'chelsea']) {
    const entry = manifest.entries.find((item) => item.id === id);
    assert(entry && entry.license === 'CC0-1.0' && entry.category === 'photo');
    assert.equal(entry.sha256, run.inputs[id].sha256);
    refBytes[id] = await reference(id, path.join(root, entry.path), entry.sha256,
      { width: 320, height: 320 }, references);
    references[id].source = entry.source;
    references[id].license = entry.license;
  }
  const cases = [];
  for (const item of run.cases) {
    assert(['coffee', 'chelsea'].includes(item.inputId));
    assert(['jpeg', 'webp'].includes(item.format));
    assert(['none', '80%', '50%'].includes(item.condition));
    assert.equal(item.options.profile, 'upload-safe');
    assert.equal(item.options.minQuality, 45);
    assert.equal(item.options.maxQuality, 85);
    assert.equal(item.options.fit, 'inside');
    assert.equal(item.options.maxWidth, 320);
    assert.equal(item.options.maxHeight, 320);
    const baseline = run.cases.find((candidate) => candidate.inputId === item.inputId &&
      candidate.format === item.format && candidate.condition === 'none');
    assert(baseline);
    const expectedTarget = item.condition === 'none' ? null : baseline.ok
      ? Math.floor(baseline.output.bytes * (item.condition === '80%' ? 80 : 50) / 100) : null;
    assert.equal(item.targetBytes, expectedTarget);
    let output = null;
    let quality;
    if (item.ok) {
      const outputFile = path.join(target, item.output.file);
      const bytes = await readChecked(outputFile, item.output.sha256);
      assert.equal(bytes.length, item.output.bytes);
      const meta = await pixelMetadata(bytes);
      assert.equal(meta.width, item.output.width);
      assert.equal(meta.height, item.output.height);
      assert.equal(meta.format, item.format);
      assert.equal(item.result.bytesOut, bytes.length);
      assert.equal(item.result.metrics.qualityUsed, item.result.qualityUsed);
      assert.equal(item.result.budgetMet, expectedTarget === null || bytes.length <= expectedTarget);
      output = { path: relative(outputFile), sha256: item.output.sha256,
        bytes: bytes.length, width: meta.width, height: meta.height, format: meta.format };
      quality = await score(refBytes[item.inputId], bytes);
    } else {
      const reason = item.error?.message || 'image not returned';
      quality = { status: 'unmeasured', reason,
        ssim: { status: 'unmeasured', value: null, reason },
        psnr: { status: 'unmeasured', value: null, reason } };
    }
    cases.push({ id: `${item.inputId}-${item.format}-${item.condition}`, inputId: item.inputId,
      category: 'photo', source: references[item.inputId].source,
      license: references[item.inputId].license,
      imageGeneration: { rawEvidence: relative(file), rawEvidenceSha256: rawHash,
        generatedAt: run.generatedAt, imageGenerationCodeSha: run.imageGenerationCodeSha,
        runtime: 'Chrome Worker default load', installedPackage: run.package.tarballIntegrity,
        policy: item.options },
      input: references[item.inputId].input, reference: references[item.inputId].reference,
      output, format: item.format, condition: item.condition, targetBytes: item.targetBytes,
      budgetMet: item.ok ? item.result.budgetMet : null,
      qualityUsed: item.ok ? item.result.metrics.qualityUsed : null,
      quality, failureReason: item.ok ? null : item.error?.message || 'image not returned' });
  }
  for (const row of cases) {
    const base = cases.find((item) => item.inputId === row.inputId && item.format === row.format && item.condition === 'none');
    row.vsNoBudget = row.condition === 'none' ? null : {
      bytesDelta: row.output && base.output ? row.output.bytes - base.output.bytes : null,
      bytesRatio: row.output && base.output ? row.output.bytes / base.output.bytes : null,
      ssimDelta: row.quality.ssim.status === 'measured' && base.quality.ssim.status === 'measured'
        ? row.quality.ssim.value - base.quality.ssim.value : null,
      psnrDeltaDb: row.quality.psnr.status === 'measured' && base.quality.psnr.status === 'measured'
        ? row.quality.psnr.value - base.quality.psnr.value : null };
  }
  return cases;
}

async function visuals(cases) {
  const cropFiles = [];
  for (const row of cases) {
    if (!row.output || row.quality.status !== 'measured') continue;
    const region = crops[row.inputId];
    assert(region);
    for (const [role, file] of [['reference', row.reference.path], ['output', row.output.path]]) {
      const name = `${row.id.replaceAll('%', 'pct')}-${role}-crop.png`;
      const destination = path.join(target, name);
      await sharp(path.join(root, file)).extract(region).png().toFile(destination);
      cropFiles.push({ caseId: row.id, role, file: name, region,
        sha256: await shaFile(destination) });
    }
  }
  const escape = (value) => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;')
    .replaceAll('"', '&quot;');
  const html = ['<!doctype html><html lang="ja"><meta charset="utf-8">',
    '<title>Wasm v1.4.0 品質比較</title>',
    '<style>body{font:16px system-ui;max-width:1100px;margin:auto;padding:20px}section{border-top:1px solid #aaa;padding:20px 0}.pair{display:flex;gap:20px;flex-wrap:wrap}.pair figure{margin:0;width:48%;min-width:300px}.pair img{max-width:100%;height:auto}.crop img{width:288px;height:288px;image-rendering:pixelated}figcaption{font-weight:600}small{display:block}</style>',
    '<h1>公開Wasm v1.4.0 固定参照との比較</h1>',
    '<p>左は元入力から作ったlossless resize参照、右は公開Wasm出力。各組のcropは両側同一座標で切り出し、どちらも表示幅288 pxとした。目視資料であり主観評価実験ではない。</p>'];
  for (const row of cases) {
    if (!row.output || row.quality.status !== 'measured') continue;
    const refCrop = cropFiles.find((item) => item.caseId === row.id && item.role === 'reference');
    const outCrop = cropFiles.find((item) => item.caseId === row.id && item.role === 'output');
    const ref = escape(path.relative(target, path.join(root, row.reference.path)));
    const out = escape(path.relative(target, path.join(root, row.output.path)));
    html.push(`<section><h2>${escape(row.id)}</h2><small>${escape(row.category)} / ${escape(row.output.width)}×${escape(row.output.height)} / crop ${escape(JSON.stringify(refCrop.region))}</small>`,
      `<div class="pair"><figure><figcaption>参照</figcaption><img src="${ref}"></figure><figure><figcaption>出力</figcaption><img src="${out}"></figure></div>`,
      `<div class="pair crop"><figure><figcaption>参照 crop</figcaption><img src="${escape(refCrop.file)}"></figure><figure><figcaption>出力 crop</figcaption><img src="${escape(outCrop.file)}"></figure></div></section>`);
  }
  html.push('</html>');
  await fs.writeFile(path.join(target, 'visual-comparison.html'), html.join('\n'));
  return cropFiles;
}

function metric(item, digits) {
  if (item.status === 'perfect') return '∞';
  return item.status === 'measured' ? item.value.toFixed(digits) : `${item.status}: ${item.reason}`;
}

function render(report) {
  const lines = ['# 公開Wasm v1.4.0 品質評価', '',
    `解析日時: ${report.analyzedAt} / 解析コード: \`${report.analysisCodeSha}\`.`,
    `方法: [固定手順](PROTOCOL.md)。raw: [写真の公開npm実行](photo-run-evidence.json)、[ケース別JSON](quality-results.json)、[並列画像・同座標crop](visual-comparison.html)、[目視所見](OBSERVATIONS.md)。`,
    '', '測定完了と画質の十分性は別判定。品質の合格閾値は設定していない。SSIM/PSNRは処理全体の出力と固定lossless参照との差。',
    '', '## 保存済み出力（3 runtimeで同一bytes、採点は4標本）', '',
    '| ケース | target / bytes | budget | quality | SSIM | PSNR dB |',
    '|---|---:|---|---:|---:|---:|'];
  for (const row of report.cases.filter((item) => item.category === 'synthetic')) {
    lines.push(`| ${row.id} | ${row.targetBytes} / ${row.output.bytes} | ${row.budgetMet} | ${row.qualityUsed ?? '未記録'} | ${metric(row.quality.ssim, 5)} | ${metric(row.quality.psnr, 2)} |`);
  }
  lines.push('', '各runtimeの元実行日時・コードSHA・raw evidence hash・出力画像はJSONの`imageGeneration`に対応付けた。10 B best-effortの選択qualityは当時のrawに未記録。',
    '', '## 実写真（Chrome Worker通常ロード、12ケース）', '',
    '| 入力 | 形式 | 条件 | target / bytes | budget | quality | SSIM | PSNR dB | bytes差 | SSIM差 | PSNR差 dB |',
    '|---|---|---|---:|---|---:|---:|---:|---:|---:|---:|');
  for (const row of report.cases.filter((item) => item.category === 'photo')) {
    lines.push(`| ${row.inputId} | ${row.format} | ${row.condition} | ${row.targetBytes ?? 'なし'} / ${row.output?.bytes ?? '画像なし'} | ${row.budgetMet ?? 'n/a'} | ${row.qualityUsed ?? 'n/a'} | ${metric(row.quality.ssim, 5)} | ${metric(row.quality.psnr, 2)} | ${row.vsNoBudget?.bytesDelta ?? '—'} | ${row.vsNoBudget?.ssimDelta?.toFixed(5) ?? '—'} | ${row.vsNoBudget?.psnrDeltaDb?.toFixed(2) ?? '—'} |`);
  }
  lines.push('', 'bytes差・SSIM差・PSNR差は同じ入力・形式のbudgetなしを基準にした補助値。JPEGとWebPのquality数値を相互比較しない。',
    '', '## 方法と限界', '',
    '- 参照: sharp 0.35.0 / libvips 8.18.3、EXIF autoOrient、入力ICC→sRGB、inside/Lanczos3、拡大なし、PNG。入力・参照・出力のhashと寸法はJSON。',
    '- PSNR: 不透明sRGB 8-bit RGB、alphaは誤差平均から除外。既存native向けRGBA helperは変更していない。',
    '- SSIM: ssim.js 3.5.0、Weber、window 8、integer grayscale、8 bit、原実装の内部downsample条件をJSONに記録。負値は保持する。',
    '- decode、ICC処理、resize、codecの差を含む。2写真と固定の合成fixtureから一般的upload画像の品質維持・知覚的同等性は保証しない。',
    '- 目視資料は同一座標・倍率の比較であり主観評価実験ではない。');
  return `${lines.join('\n')}\n`;
}

async function main() {
  const sha = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
  const dirty = execFileSync('git', ['status', '--porcelain', '--', ...sourceFiles],
    { cwd: root, encoding: 'utf8' }).trim();
  assert.equal(dirty, '', 'analysis code must be committed before analysis');
  await fs.mkdir(target, { recursive: true });
  const references = {};
  const cases = [...await savedCases(references), ...await photoCases(references)];
  assert.equal(cases.length, 16);
  const cropFiles = await visuals(cases);
  const report = { analyzedAt: new Date().toISOString(), analysisCodeSha: sha,
    analysisSourceHashes: Object.fromEntries(await Promise.all(sourceFiles.map(async (file) =>
      [file, await shaFile(path.join(root, file))]))),
    command: 'node test/benchmarks/wasm-quality-analyze.mjs',
    scoring: { psnr: '8-bit sRGB RGB MSE; 10log10(255^2/MSE); alpha excluded',
      ssim: { package: 'ssim.js', version: '3.5.0', options: ssimOptions },
      sharpVersions: sharp.versions, qualityThreshold: null },
    references, cases, cropFiles,
    measurementCompleted: cases.every((item) => item.quality.status === 'measured'),
    qualitySufficient: 'not assessed; no threshold or subjective study' };
  await fs.writeFile(path.join(target, 'quality-results.json'), `${JSON.stringify(report, null, 2)}\n`);
  await fs.writeFile(path.join(target, 'RESULTS.md'), render(report));
  console.log(`Saved ${cases.length} rows; measured ${cases.filter((item) => item.quality.status === 'measured').length}`);
}

main().catch((error) => { console.error(error.stack || error.message); process.exitCode = 1; });
