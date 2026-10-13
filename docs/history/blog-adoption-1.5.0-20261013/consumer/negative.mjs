import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
const out=path.resolve(process.argv[2]);fs.mkdirSync(out);const hash=b=>createHash('sha256').update(b).digest('hex');
function snapshot(dir){if(!fs.existsSync(dir))return [];return fs.readdirSync(dir).sort().flatMap(name=>{const file=path.join(dir,name);return fs.statSync(file).isDirectory()?snapshot(file).map(x=>({...x,path:`${name}/${x.path}`})):[{path:name,sha256:hash(fs.readFileSync(file))}];});}
fs.writeFileSync('inputs/not-image.txt','not an image');
fs.writeFileSync('policies/negative-strict.json',JSON.stringify({widths:[64],formats:['webp'],placeholder:true,budgets:[{width:64,format:'webp',maxBytes:1,maxQuality:30}]}));
fs.writeFileSync('negative-invalid.json',JSON.stringify([{id:'not-image',file:'inputs/not-image.txt',A:'policies/A-coffee.json',B:'policies/A-coffee.json'}]));
fs.writeFileSync('negative-strict.json',JSON.stringify([{id:'coffee-64',file:'inputs/coffee-64.png',sha256:hash(fs.readFileSync('inputs/coffee-64.png')),A:'policies/negative-strict.json',B:'policies/negative-strict.json'}]));
const rows=[];
for(const [name,condition,site,config] of [['invalid','A',path.join(out,'invalid'),'negative-invalid.json'],['strict','B',path.join(out,'strict'),'negative-strict.json'],['existing','B',path.resolve(process.argv[3]||'../runs/B1'),'inputs.json']]){
 const before=snapshot(site),reportFile=path.join(out,`${name}.json`),args=['build.mjs',condition,site,reportFile,config],result=spawnSync(process.execPath,args,{encoding:'utf8'});
 fs.writeFileSync(path.join(out,`${name}.stdout.log`),result.stdout??'');fs.writeFileSync(path.join(out,`${name}.stderr.log`),result.stderr??'');fs.writeFileSync(path.join(out,`${name}.exit`),`${result.status}\n`);
 assert.equal(result.status,1);const report=JSON.parse(fs.readFileSync(reportFile));assert.equal(report.successPageCreated,false);assert.equal(report.success,false);const after=snapshot(site);if(name==='existing')assert.deepEqual(after,before);else assert.ok(!after.some(x=>x.path==='index.html'));
 const siblings=fs.readdirSync(path.dirname(site)).filter(n=>n.startsWith(`.${path.basename(site)}.lazy-image-`));
 const imageSiblings=fs.existsSync(path.join(site,'images'))?fs.readdirSync(path.join(site,'images')).filter(n=>n.startsWith('.')):[];
 rows.push({name,command:[process.execPath,...args],exitCode:result.status,signal:result.signal,error:result.error?.message??null,observedError:report.applicationError??report.rows.find(x=>x.error)?.error,successPageCreated:report.successPageCreated,before,after,remainingSiteSiblingStaging:siblings,remainingImageParentStaging:imageSiblings});
}
fs.writeFileSync(path.join(out,'summary.json'),JSON.stringify(rows,null,2)+'\n');console.log(JSON.stringify(rows.map(({name,exitCode,observedError,after,remainingImageParentStaging})=>({name,exitCode,observedError,files:after.length,remainingImageParentStaging})),null,2));
