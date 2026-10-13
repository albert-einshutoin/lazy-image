import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { ImageEngine, inspect } from '@alberteinshutoin/lazy-image';
const require=createRequire(import.meta.url),root=process.argv[2];
const hash=b=>createHash('sha256').update(b).digest('hex');
for(const key of ['NAPI_RS_NATIVE_LIBRARY_PATH','NODE_PATH','NODE_OPTIONS'])assert.ok(!process.env[key],`unexpected loader override: ${key}`);
const main=require.resolve('@alberteinshutoin/lazy-image'),binding=Object.keys(require.cache).find(f=>f.endsWith('.node'));
assert.ok(main.includes('/consumer/node_modules/'));assert.ok(binding.includes('/consumer/node_modules/@alberteinshutoin/lazy-image-darwin-arm64/'));
const pkg=JSON.parse(fs.readFileSync(path.join(path.dirname(main),'package.json')));assert.equal(pkg.version,'1.5.0');
const corpus=JSON.parse(fs.readFileSync(path.join(root,'test/benchmarks/corpus/release-manifest.json')));
fs.mkdirSync('inputs');fs.mkdirSync('policies');fs.copyFileSync(path.join(root,'LICENSE'),'LICENSE');fs.copyFileSync(path.join(root,'test/benchmarks/corpus/CC0-1.0.txt'),'CC0-1.0.txt');
const inputs=[];
for(const id of ['coffee','chelsea','ui-1','metadata-1']){
 const entry=corpus.entries.find(x=>x.id===id),bytes=fs.readFileSync(path.join(root,entry.path));assert.equal(hash(bytes),entry.sha256);assert.equal(bytes.length,entry.bytes);
 const file=`inputs/${path.basename(entry.path)}`;fs.writeFileSync(file,bytes);inputs.push({id,file,source:entry.source,license:entry.license,originalPath:entry.path});
}
const large='test/fixtures/test_3.2MB_5000x5000.jpg';fs.copyFileSync(path.join(root,large),'inputs/large.jpg');inputs.push({id:'large',file:'inputs/large.jpg',source:{kind:'project-synthetic-fixture',path:large},license:'MIT',originalPath:large});
await ImageEngine.fromPath('inputs/coffee.png').resize({width:64}).toFile('inputs/coffee-64.png','png');
inputs.push({id:'coffee-64',file:'inputs/coffee-64.png',source:{kind:'derived-from-coffee',parentSha256:corpus.entries.find(x=>x.id==='coffee').sha256,command:"ImageEngine.fromPath('inputs/coffee.png').resize({width:64}).toFile('inputs/coffee-64.png','png')",packageVersion:'1.5.0'},license:'CC0-1.0'});
const requested=[320,640,960];
for(const input of inputs){
 const bytes=fs.readFileSync(input.file),facts=inspect(bytes);input.bytes=bytes.length;input.sha256=hash(bytes);input.inspection=facts;
 const swap=facts.orientation>=5;input.displayWidth=swap?facts.height:facts.width;input.displayHeight=swap?facts.width:facts.height;
 const widths=requested.filter(w=>w<=input.displayWidth);input.actualWidths=widths.length?widths:[input.displayWidth];
 const A={widths:requested,formats:['webp','jpeg'],placeholder:true};
 const B={...A,budgets:input.actualWidths.flatMap(width=>{
  const bucket=requested.find(w=>width<=w);const i=requested.indexOf(bucket);
  return [{width,format:'webp',maxBytes:[40,100,200][i]*1024,maxQuality:80},{width,format:'jpeg',maxBytes:[60,150,300][i]*1024,maxQuality:85}];
 })};
 for(const [condition,policy] of Object.entries({A,B})){const name=`policies/${condition}-${input.id}.json`;fs.writeFileSync(name,JSON.stringify(policy,null,2)+'\n');input[condition]=name;}
}
fs.writeFileSync('inputs.json',JSON.stringify(inputs,null,2)+'\n');
const lock=JSON.parse(fs.readFileSync('package-lock.json'));
const installedFiles=[main,path.join(path.dirname(main),'index.mjs'),...['artifact-policy.js','artifact-compiler.js'].map(f=>path.join(path.dirname(main),'lib',f)),binding];
const identity={package:pkg.name,version:pkg.version,publishedSource:'023a8c9368eef58f17ecc6f412363d24e5aa9b67',evaluationBase:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),os:{platform:os.platform(),release:os.release(),arch:os.arch(),cpu:os.cpus()[0].model,cpuCount:os.cpus().length,ramBytes:os.totalmem(),macOS:execFileSync('sw_vers',['-productVersion'],{encoding:'utf8'}).trim()},node:process.version,npm:execFileSync('npm',['--version'],{encoding:'utf8'}).trim(),registry:'https://registry.npmjs.org/',loaderOverrides:'absent',installed:installedFiles.map(file=>({file,sha256:hash(fs.readFileSync(file))})),registryPackages:Object.entries(lock.packages).filter(([key])=>key.includes('@alberteinshutoin/')).map(([key,value])=>({key,...value})),licenses:[{file:'LICENSE',sha256:hash(fs.readFileSync('LICENSE'))},{file:'CC0-1.0.txt',sha256:hash(fs.readFileSync('CC0-1.0.txt'))}]};
fs.writeFileSync('../identity.json',JSON.stringify(identity,null,2)+'\n');console.log(JSON.stringify(inputs.map(({id,bytes,sha256,inspection,actualWidths})=>({id,bytes,sha256,inspection,actualWidths})),null,2));
