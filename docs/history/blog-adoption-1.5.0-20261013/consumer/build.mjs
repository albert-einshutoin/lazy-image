import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { compileImage } from '@alberteinshutoin/lazy-image';
const require=createRequire(import.meta.url),hash=b=>createHash('sha256').update(b).digest('hex');
const here=path.dirname(fileURLToPath(import.meta.url));
const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const captions={coffee:'コーヒー：CC0の写真',chelsea:'猫：異なるCC0の写真','ui-1':'文字と細線：プロジェクト生成の合成図','metadata-1':'Orientation=6：metadata確認用の合成図',large:'5000×5000：大入力の合成fixture','coffee-64':'64px：coffeeからの派生画像（独立した写真ではありません）'};
export function renderPage(rows){
 const figures=rows.map(({id,manifest})=>{
  const artifacts=format=>manifest.artifacts.filter(a=>a.format===format);
  const url=relative=>`images/${encodeURIComponent(id)}/${relative.split('/').map(encodeURIComponent).join('/')}`;
  const set=format=>artifacts(format).map(a=>`${url(a.path)} ${a.width}w`).join(', ');
  const largest=artifacts('jpeg').at(-1),width=largest.width;
  const sizes=`(max-width: ${width+48}px) calc(100vw - 48px), ${width}px`;
  const placeholder=manifest.placeholder?` style="background-image:url('${url(manifest.placeholder.path)}');background-size:cover"`:'';
  return `<section><h2>${escape(captions[id]||id)}</h2><p>記事の内容を補う画像を、実manifestから配置します。出力幅：${[...new Set(manifest.artifacts.map(a=>a.width))].join(' / ')}px。</p><picture><source type="image/webp" srcset="${set('webp')}" sizes="${sizes}"><img data-image-id="${escape(id)}" src="${url(largest.path)}" srcset="${set('jpeg')}" sizes="${sizes}" width="${largest.width}" height="${largest.height}" alt="${escape(captions[id]||id)}"${placeholder}></picture><p><a href="images/${encodeURIComponent(id)}/manifest.json">画像のmanifest</a></p></section>`;
 }).join('\n');
 return `<!doctype html><html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>画像6点を含む技術ブログ記事</title><style>body{margin:0;background:#faf9f6;color:#20252b;font:16px/1.8 system-ui,sans-serif}main{max-width:960px;margin:32px auto;padding:24px}h1{line-height:1.35}section{margin:40px 0}img{display:block;max-width:100%;height:auto}a{color:#165b93}</style></head><body><main><h1>manifestで組み立てる静的な技術ブログ</h1><p>画像を事前に処理し、ページは確定した成果物だけを参照します。URLとレスポンシブ画像の幅は実際のmanifestから取得しています。</p><p>写真、文字と細線、Orientation、大入力、小画像の違いを、同じページ内で確認します。これは既存素材による用途再現です。</p>${figures}<footer>検証用のローカル記事。実利用者の採用実績ではありません。</footer></main></body></html>\n`;
}
function errorFacts(error){return {name:error.name,message:error.message,phase:error.phase??null,errorCode:error.errorCode??null,code:error.code??null,recoveryHint:error.recoveryHint??null,cause:error.cause?errorFacts(error.cause):null,cleanupError:error.cleanupError?errorFacts(error.cleanupError):null};}
export async function build(condition,site,config='inputs.json'){
 const started=performance.now(),report={condition,packageVersion:'1.5.0',rows:[],nativeTotalMs:null,nativeMetricsReason:'compileImage public manifest does not expose native metrics',successPageCreated:false};
 try{
  for(const key of ['NAPI_RS_NATIVE_LIBRARY_PATH','NODE_PATH','NODE_OPTIONS'])assert.ok(!process.env[key],`unexpected loader override: ${key}`);
  const entry=require.resolve('@alberteinshutoin/lazy-image');assert.equal(JSON.parse(fs.readFileSync(path.join(path.dirname(entry),'package.json'))).version,'1.5.0');
  assert.ok(['A','B'].includes(condition));fs.mkdirSync(site);fs.mkdirSync(path.join(site,'images'));
  for(const input of JSON.parse(fs.readFileSync(path.resolve(here,config)))){
   const policyPath=path.resolve(here,input[condition]),policy=JSON.parse(fs.readFileSync(policyPath));
   const row={id:input.id,inputSha256:input.sha256,policy,policySha256:hash(fs.readFileSync(policyPath))},time=performance.now();
   try{row.manifest=await compileImage({inputPath:path.resolve(here,input.file),outputDir:path.join(site,'images',input.id),policy});row.success=true;}
   catch(error){row.success=false;row.error=errorFacts(error);}
   row.wallMs=performance.now()-time;report.rows.push(row);
  }
  if(report.rows.every(row=>row.success)){
   fs.writeFileSync(path.join(site,'index.html'),renderPage(report.rows),{flag:'wx'});report.successPageCreated=true;
  }
  report.success=report.successPageCreated;
 }catch(error){report.success=false;report.applicationError=errorFacts(error);}
 report.pageWallMs=performance.now()-started;return report;
}
if(process.argv[1]===fileURLToPath(import.meta.url)){
 if(process.argv[2]==='--self-test'){
  const manifest={artifacts:[{format:'jpeg',width:64,height:43,path:'image-64.jpg'},{format:'webp',width:64,height:43,path:'image-64.webp'}],placeholder:{path:'placeholder.webp'}};
  const html=renderPage([{id:'coffee-64',manifest}]);assert.ok(html.includes('64w')&&html.includes(', 64px"'));assert.ok(!html.includes('320w')&&!html.includes('960w'));assert.ok(html.includes('type="image/webp"'));assert.ok(html.includes('src="images/coffee-64/image-64.jpg"'));assert.ok(html.includes("url('images/coffee-64/placeholder.webp')"));console.log('PASS: actual manifest URL/width, small sizes, WebP source and JPEG fallback');
 }else{
  const [condition,site,reportFile,config]=process.argv.slice(2);const report=await build(condition,path.resolve(site),config);fs.writeFileSync(reportFile,JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({condition,success:report.success,pageWallMs:report.pageWallMs,images:report.rows.map(r=>({id:r.id,success:r.success,wallMs:r.wallMs,error:r.error??null})),applicationError:report.applicationError??null}));process.exitCode=report.success?0:1;
 }
}
