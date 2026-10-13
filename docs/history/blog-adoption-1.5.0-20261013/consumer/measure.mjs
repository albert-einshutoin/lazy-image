import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const here=path.dirname(fileURLToPath(import.meta.url)),out=path.resolve(process.argv[2]),hash=b=>createHash('sha256').update(b).digest('hex');
fs.mkdirSync(out);
const revision=process.env.EVALUATION_REVISION;if(!revision)throw new Error('EVALUATION_REVISION required for recorded runs');
const codeHashes=['build.mjs','measure.mjs','prepare.mjs'].map(file=>({file,sha256:hash(fs.readFileSync(path.join(here,file)))}));
const rows=[];
for(let trial=1;trial<=3;trial++)for(const condition of ['A','B']){
 const name=`${condition}${trial}`,site=path.join(out,name),reportFile=path.join(out,`${name}.json`),args=[path.join(here,'build.mjs'),condition,site,reportFile];
 const started=performance.now(),result=spawnSync(process.execPath,args,{cwd:here,encoding:'utf8'}),wall=performance.now()-started;
 fs.writeFileSync(path.join(out,`${name}.stdout.log`),result.stdout??'');fs.writeFileSync(path.join(out,`${name}.stderr.log`),result.stderr??'');fs.writeFileSync(path.join(out,`${name}.exit`),`${result.status}\n`);
 const observed=fs.existsSync(reportFile)?JSON.parse(fs.readFileSync(reportFile)):null;
 const files=[];function walk(dir){if(!fs.existsSync(dir))return;for(const item of fs.readdirSync(dir,{withFileTypes:true})){const file=path.join(dir,item.name);if(item.isDirectory())walk(file);else{const bytes=fs.readFileSync(file);files.push({path:path.relative(site,file),bytes:bytes.length,sha256:hash(bytes)});}}}walk(site);
 const row={name,condition,trial,command:[process.execPath,...args],evaluationRevision:revision,codeHashes,processWallMs:wall,exitCode:result.status,signal:result.signal,spawnError:result.error?.message??null,pageWallMs:observed?.pageWallMs??null,success:observed?.success??false,files,generatedBytes:files.reduce((sum,f)=>sum+f.bytes,0)};
 rows.push(row);fs.writeFileSync(path.join(out,`${name}.process.json`),JSON.stringify(row,null,2)+'\n');console.log(`${name} exit=${row.exitCode} process=${wall.toFixed(1)}ms page=${row.pageWallMs?.toFixed(1)}ms generated=${row.generatedBytes}B`);
}
fs.writeFileSync(path.join(out,'processes.json'),JSON.stringify(rows,null,2)+'\n');process.exitCode=rows.every(r=>r.exitCode===0)?0:1;
