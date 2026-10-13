import fs from 'node:fs';
import path from 'node:path';
const preview=path.resolve(process.argv[2]||'../preview'),root=path.join(preview,'quality-single');fs.mkdirSync(root);
const inputs=JSON.parse(fs.readFileSync('inputs.json')),results=JSON.parse(fs.readFileSync(path.join(path.resolve(process.argv[3]||'../runs'),'B1.json')));
for(const input of inputs){
 const row=results.rows.find(x=>x.id===input.id),artifact=row.manifest.artifacts.filter(x=>x.format==='webp').at(-1),dir=path.join(root,input.id);fs.mkdirSync(dir);
 const reference=input.id==='metadata-1'?'metadata-1.png':path.basename(input.file);
 for(const [label,source,name] of [['Original',path.join(preview,'reference',reference),reference],['A',path.join(path.resolve(process.argv[3]||'../runs'),'A1/images',input.id,artifact.path),'A.webp'],['B',path.join(path.resolve(process.argv[3]||'../runs'),'B1/images',input.id,artifact.path),'B.webp']])fs.copyFileSync(source,path.join(dir,name));
 fs.copyFileSync(path.join(preview,'qa.js'),path.join(dir,'qa.js'));
 const figures=[['Original',reference],['A','A.webp'],['B','B.webp']].map(([label,file])=>`<figure><figcaption>${label}</figcaption><div class="crop"><img src="${file}" alt="${input.id} ${label}" style="width:${artifact.width}px;height:${artifact.height}px;max-width:none"></div></figure>`).join('');
 fs.writeFileSync(path.join(dir,'index.html'),`<!doctype html><html lang="ja"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${input.id} 原画像・A・B品質比較</title><style>body{font:16px/1.5 system-ui;color:#20252b;background:#faf9f6;margin:24px}main{width:1060px;margin:auto}.row{display:flex;gap:20px}figure{margin:0;width:320px}.crop{width:320px;height:320px;overflow:hidden;display:flex;justify-content:center;align-items:center;background:#eee}.crop img{flex-shrink:0}details{margin:40px auto}</style><main><h1>${input.id}：原画像 / A / B</h1><p>同じ表示寸法 ${artifact.width}×${artifact.height}px、中心320px crop。metadata原図はOrientationを正規化した測定外PNG。これはAgentの目視であり、利用者本人の承認ではありません。</p><div class="row">${figures}</div><details><summary>品質表示の観測</summary><pre id="qa-observation">loading</pre></details></main><script src="qa.js"></script></html>`);
}
console.log('generated six exact-dimension, same-byte quality triplets; no primary measurements changed');
