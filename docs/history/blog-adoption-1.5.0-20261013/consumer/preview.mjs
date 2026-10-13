import fs from 'node:fs';
import path from 'node:path';
import { ImageEngine } from '@alberteinshutoin/lazy-image';
const out=path.resolve(process.argv[2]),runs=path.resolve(process.argv[3]||'../runs');fs.mkdirSync(out);
for(const viewport of ['desktop','mobile'])for(const condition of ['A','B'])fs.cpSync(path.join(runs,`${condition}1`),path.join(out,`${viewport}-${condition}`),{recursive:true});
fs.mkdirSync(path.join(out,'reference'));
const inputs=JSON.parse(fs.readFileSync('inputs.json'));
for(const input of inputs){
 const reference=input.id==='metadata-1'?`${input.id}.png`:path.basename(input.file);
 if(input.id==='metadata-1')await ImageEngine.fromPath(input.file).autoOrient(true).toFile(path.join(out,'reference',reference),'png');else fs.copyFileSync(input.file,path.join(out,'reference',reference));
 input.reference=reference;
}
const b=JSON.parse(fs.readFileSync(path.join(runs,'B1.json')));
// JPEG-only copy is a browser diagnostic, outside measured page build/capacity.
const article=fs.readFileSync(path.join(out,'desktop-B/index.html'),'utf8');fs.writeFileSync(path.join(out,'desktop-B/qa-jpeg.html'),article.replace(/<source\b[^>]*>/g,''));
let figures='';
for(const input of inputs){
 const row=b.rows.find(x=>x.id===input.id),artifact=row.manifest.artifacts.filter(a=>a.format==='webp').at(-1),width=artifact.width;
 const cells=[['Original',`reference/${input.reference}`],['A',`desktop-A/images/${input.id}/${artifact.path}`],['B',`desktop-B/images/${input.id}/${artifact.path}`]];
 figures+=`<section><h2>${input.id}</h2><p>同じ表示幅${width}pxの中心crop。metadata原画像は公開ImageEngine.autoOrient(true)→PNGで参照方向を正規化（測定外）。</p><div class="row">${cells.map(([label,url])=>`<figure><figcaption>${label}</figcaption><div class="crop"><img src="${url}" style="width:${width}px;max-width:none" alt="${input.id} ${label}"></div></figure>`).join('')}</div></section>`;
}
fs.writeFileSync(path.join(out,'quality.html'),`<!doctype html><html lang="ja"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>原画像・A・Bの品質確認</title><style>body{font:16px/1.5 system-ui;color:#20252b;background:#faf9f6;margin:24px}main{width:1060px;margin:auto}.row{display:flex;gap:20px}figure{margin:0;width:320px}.crop{width:320px;height:320px;overflow:hidden;display:flex;justify-content:center;align-items:center;background:#eee}.crop img{flex-shrink:0;height:auto}section{margin-bottom:28px}</style><main><h1>同じ表示寸法での中心crop比較</h1><p>写真・合成図・大入力・派生小画像のAgent目視。利用者本人の承認ではありません。</p>${figures}</main></html>`);
console.log('created desktop/mobile A/B, JPEG-only diagnostic and original/A/B quality sheet');

fs.copyFileSync('qa.js',path.join(out,'qa.js'));
for(const name of ['desktop-A','desktop-B','mobile-A','mobile-B']){
 const target=path.join(out,name+'-observed');fs.cpSync(path.join(out,name),target,{recursive:true});fs.copyFileSync('qa.js',path.join(target,'qa.js'));
 const file=path.join(target,'index.html');fs.writeFileSync(file,fs.readFileSync(file,'utf8').replace('</body>','<details><summary>表示検証の観測（測定外）</summary><pre id="qa-observation">loading</pre></details><script src="qa.js"></script></body>'));
}
const jpeg=path.join(out,'desktop-B-observed/qa-jpeg.html');fs.writeFileSync(jpeg,fs.readFileSync(jpeg,'utf8').replace('</body>','<details><summary>JPEG fallback診断</summary><pre id="qa-observation">loading</pre></details><script src="qa.js"></script></body>'));
