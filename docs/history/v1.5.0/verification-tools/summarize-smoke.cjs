const fs=require('fs'),path=require('path'),assert=require('assert/strict');
const [dir,kind,sha]=process.argv.slice(2);
const folders=fs.readdirSync(dir).filter(n=>n.startsWith(kind==='published'?'registry-smoke-':'candidate-smoke-'));
assert.equal(folders.length,12);
const expected=new Set(['darwin-arm64','darwin-x64','linux-x64-gnu','linux-arm64-gnu','linux-x64-musl','win32-x64-msvc'].flatMap(p=>[`${p}/22`,`${p}/24`]));
const rows=folders.map(name=>{
 const file=path.join(dir,name,kind==='published'?'lazy-image-registry-smoke.json':'lazy-image-candidate-smoke.json');
 const r=JSON.parse(fs.readFileSync(file));
 assert.equal(r.status,'PASS');assert.equal(r.scriptRevision,sha);assert.equal(r.checkoutRevision,sha);assert.equal(r.package,'@alberteinshutoin/lazy-image@1.5.0');
 assert.equal(r.source,kind==='published'?'published-registry':'candidate-tarball');
 for(const key of ['binding','api','cli','artifacts','apiCliParity','maxQuality','invalidMaxQuality'])assert.equal(r.results[key],'PASS');
 assert.deepEqual(r.maxQuality.apiArtifacts,r.maxQuality.cliArtifacts);
 const capped=r.maxQuality.apiArtifacts[0];assert.equal(capped.quality,30);assert.equal(capped.targetBytes,1048576);assert.ok(capped.bytes>0&&capped.bytes<=1048576);
 assert.deepEqual(r.maxQuality.invalid.map(x=>x.maxQuality),[29,101,'30']);
 for(const entry of r.maxQuality.invalid){assert.equal(entry.api,'E400');assert.equal(entry.cli,'E400');assert.equal(entry.unpublished,true);assert.deepEqual(entry.remainingSiblingStaging,[]);}
 for(const command of r.commandResults){assert.equal(command.exitCode,command.expectedExitCode);assert.equal(command.signal,null);assert.equal(command.error,null);}
 const key=`${r.expectedPlatform}/${r.runtime.node.match(/^v(\d+)/)[1]}`;
 assert.ok(expected.delete(key),key);
 return {platform:r.expectedPlatform,node:r.runtime.node,npm:r.runtime.npm,status:r.status,source:r.source,scriptRevision:r.scriptRevision,binding:r.loaded.binding,results:r.results,maxQuality:r.maxQuality,report:path.relative(dir,file)};
});assert.equal(expected.size,0);
const summary={version:'1.5.0',sourceSha:sha,source:kind,status:'PASS',pass:12,fail:0,blocked:0,rows};
fs.writeFileSync(path.join(dir,'summary.json'),JSON.stringify(summary,null,2)+'\n');console.log(JSON.stringify({pass:12,fail:0,blocked:0,rows:rows.map(({platform,node,npm,status})=>({platform,node,npm,status}))},null,2));
