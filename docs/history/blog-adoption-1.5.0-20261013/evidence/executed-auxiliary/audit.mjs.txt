import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { inspect } from '@alberteinshutoin/lazy-image';
const runs=path.resolve(process.argv[2]),hash=b=>createHash('sha256').update(b).digest('hex');
// Same EXIF/ICC marker checks as the existing firewall tests; observational only.
function metadata(bytes,format){
 if(format==='webp'){
  const chunks=[];for(let i=12;i+8<=bytes.length;){const size=bytes.readUInt32LE(i+4);assert.ok(i+8+size<=bytes.length);chunks.push(bytes.toString('ascii',i,i+4));i+=8+size+(size%2);}
  return {exif:chunks.includes('EXIF'),xmp:chunks.includes('XMP '),icc:chunks.includes('ICCP')};
 }
 return {exif:bytes.includes(Buffer.from('Exif\0\0')),xmp:bytes.includes(Buffer.from('http://ns.adobe.com/xap/1.0/')),icc:bytes.includes(Buffer.from('ICC_PROFILE\0'))};
}
const processes=JSON.parse(fs.readFileSync(path.join(runs,'processes.json'))),images=[];
assert.deepEqual(processes.map(x=>x.name).sort(),['A1','A2','A3','B1','B2','B3']);
const fixedInputs=JSON.parse(fs.readFileSync('inputs.json'));

for(const process of processes){
 assert.equal(process.exitCode,0);assert.equal(process.spawnError,null);assert.equal(process.signal,null);
 const report=JSON.parse(fs.readFileSync(path.join(runs,`${process.name}.json`)));assert.deepEqual(report.rows.map(x=>x.id),fixedInputs.map(x=>x.id));assert.equal(report.success,true);
 for(const row of report.rows)assert.equal(row.inputSha256,fixedInputs.find(x=>x.id===row.id).sha256);

 for(const row of report.rows){
  assert.equal(row.success,true);assert.equal(row.manifest.compiler.version,'1.5.0');assert.equal(row.manifest.source.sha256,row.inputSha256);const directory=path.join(runs,process.name,'images',row.id);
  for(const artifact of [...row.manifest.artifacts,row.manifest.placeholder]){
   const bytes=fs.readFileSync(path.join(directory,artifact.path));assert.equal(bytes.length,artifact.bytes);assert.equal(hash(bytes),artifact.sha256);
   const facts=inspect(bytes);assert.equal(facts.width,artifact.width);assert.equal(facts.height,artifact.height);assert.equal(facts.format,artifact.format);
   const observed=metadata(bytes,artifact.format);assert.equal(observed.exif,false);assert.equal(observed.xmp,false);assert.equal(observed.icc,artifact.iccOutcome==='preserved');
   if(artifact.targetBytes!==undefined){assert.ok(bytes.length<=artifact.targetBytes);const budget=row.policy.budgets.find(b=>b.width===artifact.width&&b.format===artifact.format);assert.ok(budget);assert.ok(artifact.quality>=30&&artifact.quality<=budget.maxQuality);}
   images.push({run:process.name,id:row.id,artifact,...observed,inspection:facts});
  }
  const html=fs.readFileSync(path.join(runs,process.name,'index.html'),'utf8');assert.ok(!html.includes(runs)&&!html.includes('/Users/')&&!html.includes('/Volumes/'));
  assert.ok(row.manifest.artifacts.every(a=>a.width<=row.manifest.source.displayWidth));
 }
}
const distribution=values=>{const sorted=[...values].sort((a,b)=>a-b);return {values,median:sorted[1],max:sorted.at(-1)};};
const summary={conditions:Object.fromEntries(['A','B'].map(condition=>{
 const selected=processes.filter(p=>p.condition===condition);return [condition,{normalSuccess:selected.length*6,normalAttempts:18,processWallMs:distribution(selected.map(p=>p.processWallMs)),pageWallMs:distribution(selected.map(p=>p.pageWallMs)),generatedBytes:selected.map(p=>p.generatedBytes),imageWallMs:Object.fromEntries(JSON.parse(fs.readFileSync('inputs.json')).map(({id})=>[id,distribution(selected.map(p=>JSON.parse(fs.readFileSync(path.join(runs,`${p.name}.json`))).rows.find(r=>r.id===id).wallMs))]))}];
 })),checkedFiles:images.length,budgetChecks:images.filter(x=>x.artifact.targetBytes!==undefined).length,images};
const inputMetadata=fs.readFileSync('inputs/metadata-1.jpg'),exifOffset=inputMetadata.indexOf(Buffer.from('Exif\0\0'));assert.ok(exifOffset>=0);
const tiff=inputMetadata.subarray(exifOffset+6),little=tiff.toString('ascii',0,2)==='II';
const u16=offset=>little?tiff.readUInt16LE(offset):tiff.readUInt16BE(offset),u32=offset=>little?tiff.readUInt32LE(offset):tiff.readUInt32BE(offset);
const ifd=u32(4),tags=[];for(let i=0;i<u16(ifd);i++)tags.push(u16(ifd+2+12*i));
summary.metadataInput={exif:true,gpsIfdPointerPresent:tags.includes(0x8825),xmp:inputMetadata.includes(Buffer.from('http://ns.adobe.com/xap/1.0/')),icc:inputMetadata.includes(Buffer.from('ICC_PROFILE\0'))};
const a=images.filter(x=>x.run==='A1'),b=images.filter(x=>x.run==='B1');summary.identicalImageHashesAB=a.every(x=>b.some(y=>y.id===x.id&&y.artifact.path===x.artifact.path&&y.artifact.sha256===x.artifact.sha256));
summary.identicalImageHashesAllTrials=images.every(x=>a.some(y=>x.id===y.id&&x.artifact.path===y.artifact.path&&x.artifact.sha256===y.artifact.sha256));
fs.writeFileSync(path.join(runs,'summary.json'),JSON.stringify(summary,null,2)+'\n');console.log(JSON.stringify({conditions:summary.conditions,checkedFiles:summary.checkedFiles,budgetChecks:summary.budgetChecks,identicalImageHashesAB:summary.identicalImageHashesAB},null,2));
