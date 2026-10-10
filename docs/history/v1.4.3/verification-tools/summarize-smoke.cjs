const fs=require('fs'),path=require('path'),assert=require('assert/strict');
const [dir,kind,sha]=process.argv.slice(2);
const folders=fs.readdirSync(dir).filter(n=>n.startsWith(kind==='published'?'registry-smoke-':'candidate-smoke-'));
assert.equal(folders.length,12);
const expected=new Set(['darwin-arm64','darwin-x64','linux-x64-gnu','linux-arm64-gnu','linux-x64-musl','win32-x64-msvc'].flatMap(p=>[`${p}/22`,`${p}/24`]));
const rows=folders.map(name=>{
 const file=path.join(dir,name,kind==='published'?'lazy-image-registry-smoke.json':'lazy-image-candidate-smoke.json');
 const r=JSON.parse(fs.readFileSync(file));
 assert.equal(r.status,'PASS');assert.equal(r.scriptRevision,sha);assert.equal(r.checkoutRevision,sha);assert.equal(r.package,'@alberteinshutoin/lazy-image@1.4.3');
 assert.equal(r.source,kind==='published'?'published-registry':'candidate-tarball');
 for(const value of Object.values(r.results))assert.equal(value,'PASS');
 const key=`${r.expectedPlatform}/${r.runtime.node.match(/^v(\d+)/)[1]}`;
 assert.ok(expected.delete(key),key);
 return {platform:r.expectedPlatform,node:r.runtime.node,npm:r.runtime.npm,status:r.status,source:r.source,scriptRevision:r.scriptRevision,binding:r.loaded.binding,results:r.results,report:path.relative(dir,file)};
});assert.equal(expected.size,0);
const summary={version:'1.4.3',sourceSha:sha,source:kind,status:'PASS',pass:12,fail:0,blocked:0,rows};
fs.writeFileSync(path.join(dir,'summary.json'),JSON.stringify(summary,null,2)+'\n');console.log(JSON.stringify({pass:12,fail:0,blocked:0,rows:rows.map(({platform,node,npm,status})=>({platform,node,npm,status}))},null,2));
