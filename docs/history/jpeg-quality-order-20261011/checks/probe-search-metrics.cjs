const fs = require('node:fs'), cp = require('node:child_process'), path = require('node:path'), os = require('node:os'), crypto = require('node:crypto'), { performance } = require('node:perf_hooks');
const root = '/Volumes/Satechi/Developer/lazy-image', stage = process.argv[2], buildSourceSha = process.argv[3];
if (!['before', 'after'].includes(stage) || !/^[0-9a-f]{40}$/.test(buildSourceSha)) throw Error('stage/source required');
if (stage === 'after' && cp.execFileSync('git', ['rev-parse','HEAD'], {cwd:root,encoding:'utf8'}).trim() !== buildSourceSha) throw Error('after source must match HEAD');
const { ImageEngine } = require(root + '/index'), sharp = require(root + '/node_modules/sharp');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
(async () => {
  const input = await sharp({create:{width:64,height:64,channels:3,background:{r:200,g:100,b:50}}}).png().toBuffer();
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-metrics-probe-')), calls = [];
  try {
    for (const kind of ['buffer', 'file']) for (let trial = 0; trial < 3; trial++) {
      const engine = ImageEngine.from(input), options = {targetBytes:1,minQuality:1,maxQuality:100,qualityFloorPolicy:'best-effort'};
      const output = path.join(temporary, 'output.jpg'), start = performance.now();
      const result = kind === 'buffer' ? await engine.toBufferTargetBytes('jpeg', options) : await engine.toFileTargetBytes(output, 'jpeg', options);
      const elapsedMs = performance.now() - start, data = kind === 'buffer' ? result.data : fs.readFileSync(output);
      calls.push({kind,trial,elapsedMs,quality:result.quality,bytes:data.length,budgetMet:result.budgetMet,sha256:hash(data),metrics:result.metrics});
    }
  } finally { fs.rmSync(temporary, {recursive:true,force:true}); }
  const result = {stage,buildSourceSha,workingHead:cp.execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),clean:cp.execFileSync('git',['status','--porcelain'],{cwd:root,encoding:'utf8'}).trim()==='',bindingSha256:hash(fs.readFileSync(root+'/lazy-image.darwin-arm64.node')),fixtureSha256:hash(input),command:'node probe-search-metrics.cjs '+stage+' '+buildSourceSha,scope:'public API latency includes queue/helper overhead; native totalMs measures the pipeline; process-wide CPU/RSS retain existing boundaries',calls};
  fs.writeFileSync('/tmp/lazy-image-jpeg-quality-fix/metrics-'+stage+'.json',JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({stage,calls:calls.map(c=>({kind:c.kind,quality:c.quality,bytes:c.bytes,elapsedMs:c.elapsedMs,totalMs:c.metrics.totalMs,cpuTime:c.metrics.cpuTime}))}));
})().catch(error=>{console.error(error);process.exitCode=1;});
