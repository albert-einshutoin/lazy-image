'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const zlib = require('node:zlib');
const crypto = require('node:crypto');
const { createRequire } = require('node:module');
const root = process.env.COMPARE_ROOT;
const label = process.env.COMPARE_LABEL;
const revision = process.env.COMPARE_SHA;
assert(root && label && revision);
const fromRoot = createRequire(path.join(root, 'package.json'));
const sharp = fromRoot('sharp');
const binding = path.join(root, 'lazy-image.darwin-arm64.node');
process.env.NAPI_RS_NATIVE_LIBRARY_PATH = binding;
const { ImageEngine } = fromRoot('./index.js');
assert(require.cache[binding], 'expected local binding was not loaded');
assert.equal(ImageEngine, fromRoot(binding).ImageEngine);
const digest = b => crypto.createHash('sha256').update(b).digest('hex');
const read = p => fs.readFileSync(path.join(root, p));
const { pngChunk } = fromRoot('./test/helpers/png-helpers.js');
function grayRamp() {
  const w = 64, h = 16;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 0;
  const rows = Buffer.alloc((w + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) rows[y * (w + 1) + 1 + x] = (x * 4 + y) & 255;
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),pngChunk('IHDR',ihdr),pngChunk('IDAT',zlib.deflateSync(rows)),pngChunk('IEND',Buffer.alloc(0))]);
}
function pngChunks(buf) {
  const out = [];
  for (let p = 8; p + 12 <= buf.length;) {
    const len = buf.readUInt32BE(p), type = buf.toString('ascii', p + 4, p + 8), data = buf.subarray(p + 8, p + 8 + len);
    assert(p + 12 + len <= buf.length, 'truncated PNG chunk');
    out.push({type,data}); p += 12 + len;
    if (type === 'IEND') break;
  }
  return out;
}
async function pixels(buf) {
  const {data,info} = await sharp(buf).toColourspace('srgb').ensureAlpha().raw().toBuffer({resolveWithObject:true});
  assert.equal(info.channels, 4); assert.equal(info.depth, 'uchar');
  let transparent=0, semi=0, opaque=0, hiddenRgb=0;
  for (let p=0;p<data.length;p+=4) {
    const a=data[p+3];
    if (a===0) {transparent++; if(data[p]||data[p+1]||data[p+2]) hiddenRgb++;}
    else if(a===255) opaque++; else semi++;
  }
  return {width:info.width,height:info.height,hash:digest(data),transparent,semi,opaque,hiddenRgb};
}
function median(a) { const x=[...a].sort((p,q)=>p-q); return x[Math.floor(x.length/2)]; }
async function main() {
  const metaInput = read('docs/history/wasm-1.3.1/wasm-metadata-input.jpg');
  const inputMeta = await sharp(metaInput).metadata();
  assert(inputMeta.exif && inputMeta.exif.includes(Buffer.from([0x25,0x88])) && inputMeta.xmp && inputMeta.icc, 'metadata fixture incomplete');
  const cases = [
    {id:'photo', input:read('test/benchmarks/corpus/images/chelsea.png')},
    {id:'gradient', input:read('test/benchmarks/corpus/images/illustration-1.png')},
    {id:'ui', input:read('test/benchmarks/corpus/images/ui-1.png'), file:true},
    {id:'grayscale', input:grayRamp()},
    {id:'rgba', input:read('test/benchmarks/corpus/images/alpha-1.png')},
    {id:'metadata_default', input:metaInput, metadata:'strip'},
    {id:'metadata_icc', input:metaInput, metadata:'icc'},
  ];
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-833-'));
  const results = [];
  try {
    for(const c of cases) {
      const run = () => { const e=ImageEngine.from(c.input); if(c.metadata) e.autoOrient(false); if(c.metadata==='icc') e.keepMetadata({icc:true,exif:false}); return e.toBuffer('png'); };
      const start=process.hrtime.bigint(); const output=await run(); const coldMs=Number(process.hrtime.bigint()-start)/1e6;
      const warmMs=[];
      for(let i=0;i<5;i++){const t=process.hrtime.bigint(); await run(); warmMs.push(Number(process.hrtime.bigint()-t)/1e6);}
      const inputPixels=await pixels(c.input), outputPixels=await pixels(output);
      assert.equal(outputPixels.width,inputPixels.width); assert.equal(outputPixels.height,inputPixels.height);
      const outputMeta=await sharp(output).metadata();
      const chunks=pngChunks(output), iccp=chunks.find(x=>x.type==='iCCP');
      let profile=null;
      if(iccp){const z=iccp.data.indexOf(0); assert(z>=0 && iccp.data[z+1]===0); profile=zlib.inflateSync(iccp.data.subarray(z+2));}
      if(c.metadata){
        assert(!outputMeta.exif && !outputMeta.xmp, `${c.id}: EXIF/XMP retained`);
        assert(!chunks.some(x=>x.type==='eXIf' || x.data.includes(Buffer.from('XML:com.adobe.xmp'))), `${c.id}: metadata chunks retained`);
        if(c.metadata==='strip') assert(!outputMeta.icc && !profile, 'default ICC retained');
        else {assert(profile && outputMeta.icc, 'ICC missing'); assert.equal(digest(profile),digest(inputMeta.icc),'ICC bytes changed');}
      }
      let file=null;
      if(c.file){const dest=path.join(tmp,'ui.png');const written=await ImageEngine.from(c.input).toFile(dest,'png');const bytes=fs.readFileSync(dest);file={reportedBytes:written,actualBytes:bytes.length,sha256:digest(bytes),pixels:await pixels(bytes)};assert.equal(file.actualBytes,output.length);assert.equal(file.pixels.hash,outputPixels.hash);}
      results.push({id:c.id,inputBytes:c.input.length,inputSha256:digest(c.input),inputPixels,outputBytes:output.length,outputSha256:digest(output),outputPixels,pixelEqualsInput:inputPixels.hash===outputPixels.hash,outputMetadata:{exif:!!outputMeta.exif,gpsTag:!!outputMeta.exif?.includes(Buffer.from([0x25,0x88])),xmp:!!outputMeta.xmp,icc:!!outputMeta.icc,iccSha256:profile&&digest(profile),chunks:chunks.map(x=>x.type)},coldMs,warmMs,warmMedianMs:median(warmMs),warmMinMs:Math.min(...warmMs),warmMaxMs:Math.max(...warmMs),file});
    }
  } finally { fs.rmSync(tmp,{recursive:true,force:true}); }
  const result={label,revision,runtime:{platform:process.platform,arch:process.arch,node:process.version,sharp:JSON.parse(fs.readFileSync(path.join(root,'node_modules/sharp/package.json'))).version,binding,bindingSha256:digest(fs.readFileSync(binding)),rust:'1.88.0',sdk:'/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.5.sdk',normalization:'sharp 0.35.4 toColourspace(srgb) ensureAlpha raw 8-bit uchar',timing:'ImageEngine.from(input) through awaited toBuffer(png), first cold then 5 sequential warm; process.hrtime.bigint'},metadataInput:{sha256:digest(metaInput),exif:!!inputMeta.exif,gpsTag:true,xmp:!!inputMeta.xmp,iccSha256:digest(inputMeta.icc)},cases:results};
  const dest=path.join(os.tmpdir(),`lazy-image-833-${label}.json`);fs.writeFileSync(dest,JSON.stringify(result,null,2)+'\n');console.log(dest);for(const c of results)console.log(c.id,c.outputBytes,c.warmMedianMs.toFixed(2),c.pixelEqualsInput,c.outputPixels.hiddenRgb);
}
main().catch(e=>{console.error(e);process.exitCode=1});
