import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';
const [candidateDir,wasmCandidate,outputDir]=process.argv.slice(2);
fs.mkdirSync(outputDir,{recursive:true});
const hash=b=>createHash('sha256').update(b).digest('hex');
const json=(cmd,args,cwd)=>JSON.parse(execFileSync(cmd,args,{cwd,encoding:'utf8'}));
const manifest=JSON.parse(fs.readFileSync(path.join(candidateDir,'manifest.json')));
assert.equal(manifest.version,'1.4.2');
const rows=[];
for(const name of [...manifest.packages.map(p=>p.name),'@alberteinshutoin/lazy-image-wasm']) {
 const metadata=json('npm',['view',`${name}@1.4.2`,'--json','--prefer-online','--registry=https://registry.npmjs.org/']);
 const tags=json('npm',['view',name,'dist-tags','--json','--prefer-online','--registry=https://registry.npmjs.org/']);
 assert.equal(metadata.version,'1.4.2'); assert.equal(tags.latest,'1.4.2');
 assert.ok(metadata.dist.tarball.startsWith('https://registry.npmjs.org/'));
 const response=await fetch(metadata.dist.tarball,{signal:AbortSignal.timeout(30000)}); assert.equal(response.status,200);
 const bytes=Buffer.from(await response.arrayBuffer());
 const [algorithm,expected]=metadata.dist.integrity.split('-');
 assert.equal(createHash(algorithm).update(bytes).digest('base64'),expected);
 const tarball=path.join(outputDir,path.basename(new URL(metadata.dist.tarball).pathname)); fs.writeFileSync(tarball,bytes);
 const item=manifest.packages.find(p=>p.name===name);
 if(item) {
   assert.equal(hash(fs.readFileSync(path.join(candidateDir,item.tarball))),item.tarballSha256);
   assert.equal(hash(bytes),item.tarballSha256);
 }
 rows.push({name,version:metadata.version,latest:tags.latest,registry:'https://registry.npmjs.org/',tarballUrl:metadata.dist.tarball,integrity:metadata.dist.integrity,sha256:hash(bytes),tagCandidateSha256:item?.tarballSha256??null,tagCandidateMatches:item?true:null,integrityMatches:true,tarball});
}
const wasm=rows.find(r=>r.name.endsWith('-wasm'));
function files(tarball,label) {
 const directory=path.join(outputDir,label);fs.mkdirSync(directory);
 execFileSync('tar',['-xzf',tarball,'-C',directory]);
 const list=[];
 function walk(dir) {for(const entry of fs.readdirSync(dir,{withFileTypes:true})) {const file=path.join(dir,entry.name);if(entry.isDirectory())walk(file);else {assert.ok(entry.isFile());const bytes=fs.readFileSync(file);list.push({path:path.relative(directory,file),bytes:bytes.length,sha256:hash(bytes)});}}}
 walk(directory);return list.sort((a,b)=>a.path.localeCompare(b.path));
}
const candidateFiles=files(wasmCandidate,'wasm-candidate-expanded'),publishedFiles=files(wasm.tarball,'wasm-published-expanded');
assert.deepEqual(publishedFiles,candidateFiles);
const result={checkedAt:new Date().toISOString(),tagRevision:manifest.revision,tagCandidateManifestSha256:hash(fs.readFileSync(path.join(candidateDir,'manifest.json'))),licenseSha256:manifest.licenseSha256,rows,wasm:{candidateTarballSha256:hash(fs.readFileSync(wasmCandidate)),publishedTarballSha256:wasm.sha256,archiveHashMatches:hash(fs.readFileSync(wasmCandidate))===wasm.sha256,expandedContentsMatch:true,fileCount:candidateFiles.length,candidateFiles,publishedFiles}};
fs.writeFileSync(path.join(outputDir,'registry-package-provenance.json'),JSON.stringify(result,null,2)+'\n');
console.log(`PASS: native 7/7; Wasm ${candidateFiles.length} files match; Wasm archive match=${result.wasm.archiveHashMatches}; versions/latest 8/8`);
