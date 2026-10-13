const fs = require('node:fs');
const path = require('node:path');
const [repository, phase, out] = process.argv.slice(2);
if (!repository || !phase || !out) throw Error('Usage: node preview.cjs <repository> <phase-dir> <new-preview>');
const manifest = JSON.parse(fs.readFileSync(path.join(phase, 'runs/B1/manifest.json')));
if (manifest.compiler.version !== '1.5.0' || manifest.artifacts.length !== 6) throw Error('Expected the successful B1 manifest');
fs.mkdirSync(out);
const svg = 'compiler-artifact-flow.svg';
fs.copyFileSync(path.join(repository, 'docs/images', svg), path.join(out, svg));
fs.copyFileSync(path.join(phase, 'preview/qa.js'), path.join(out, 'qa.js'));
fs.copyFileSync(path.join(phase, 'runs/B1/manifest.json'), path.join(out, 'manifest.json'));
const rows = [{ id: 'svg', file: svg, width: 960, height: 1400 }];
for (const width of [684, 960, 1920]) {
  const artifact = manifest.artifacts.find(a => a.width === width);
  const file = `control-${width}.png`;
  fs.copyFileSync(path.join(phase, 'controls', file), path.join(out, file));
  rows.push({ id: `png-${width}`, file, width, height: artifact.height });
}
for (const a of manifest.artifacts) {
  if (path.basename(a.path) !== a.path) throw Error('Unexpected artifact path');
  fs.copyFileSync(path.join(phase, 'runs/B1', a.path), path.join(out, a.path));
  rows.push({ id: `${a.format}-${a.width}`, file: a.path, width: a.width, height: a.height });
}
const style = `body{margin:0;background:white;color:#172034;font:16px Arial,sans-serif}main{max-width:1008px;box-sizing:border-box;padding:24px;margin:auto}h1{font-size:20px;margin:0 0 12px}img{display:block;width:100%;max-width:960px;height:auto}pre{white-space:pre-wrap;font-size:12px}details{margin-top:16px}.ruler{position:fixed;top:0;left:0;width:100px;height:24px;background:#000;z-index:9999;pointer-events:none}`;
function page(id, body, detail = false) {
  return `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Guide diagram: ${id}</title><style>${style}${detail ? '.pan{overflow:auto;max-height:calc(100vh - 220px);border:1px solid #94a3b8}.pan img{width:960px;max-width:none}' : ''}</style><div class="ruler" aria-hidden="true"></div><main><h1>${detail ? 'Original SVG: 960 CSS px' : id}</h1><p>${detail ? '<a href="svg.html">Normal display</a>' : '<a href="detail-svg.html">Enlarge original SVG</a>'} · <a href="picture.html">Responsive raster example</a></p>${detail ? '<p><button type="button" data-width="960">Original size</button> <button type="button" data-width="1920">2×</button> <button type="button" data-area="verification">Verification</button> <button type="button" data-area="failure">Failure</button> <button type="button" data-pan="-1">Left</button> <button type="button" data-pan="1">Right</button></p><div class="pan" role="region" aria-label="Scrollable enlarged diagram" tabindex="0">' + body + '</div>' : body}<details><summary>Observation</summary><pre id="qa-observation">loading</pre></details></main><script src="qa.js"></script>${detail ? '<script>for(const button of document.querySelectorAll("button[data-width]"))button.addEventListener("click",()=>{document.querySelector(".pan img").style.width=button.dataset.width+"px";document.querySelector("h1").textContent="Original SVG: "+button.dataset.width+" CSS px";});for(const button of document.querySelectorAll("button[data-area]"))button.addEventListener("click",()=>{const pan=document.querySelector(".pan"),scale=document.querySelector(".pan img").getBoundingClientRect().width/960;pan.scrollTo({left:0,top:(button.dataset.area==="verification"?472:1208)*scale});});for(const button of document.querySelectorAll("button[data-pan]"))button.addEventListener("click",()=>{const pan=document.querySelector(".pan");pan.scrollBy({left:Number(button.dataset.pan)*pan.clientWidth*0.8});});</script>' : ''}</html>\n`;
}
for (const row of rows) {
  const image = `<img data-image-id="${row.id}" src="${row.file}" width="${row.width}" height="${row.height}" alt="Compiler flow: input, compileImage, private staging, local commit, images and manifest, application delivery. Failure does not commit unverified artifacts.">`;
  fs.writeFileSync(path.join(out, `${row.id}.html`), page(row.id, image), { flag: 'wx' });
  if (row.id === 'svg') fs.writeFileSync(path.join(out, 'detail-svg.html'), page(row.id, image, true), { flag: 'wx' });
}
const set = format => manifest.artifacts.filter(a => a.format === format).map(a => `${a.path} ${a.width}w`).join(', ');
const largest = manifest.artifacts.find(a => a.id === '1920/jpeg');
const sizes = '(max-width: 1008px) calc(100vw - 48px), 960px';
const picture = `<picture><source type="image/webp" srcset="${set('webp')}" sizes="${sizes}"><img data-image-id="responsive-B1" src="${largest.path}" srcset="${set('jpeg')}" sizes="${sizes}" width="${largest.width}" height="${largest.height}" alt="Compiler flow diagram from the final budgeted manifest."></picture><p><a href="manifest.json">Actual compiler manifest</a></p>`;
fs.writeFileSync(path.join(out, 'picture.html'), page('Responsive B1', picture), { flag: 'wx' });
fs.writeFileSync(path.join(out, 'pages.json'), JSON.stringify(rows, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ singleImagePages: rows.length, detailPages: 1, responsivePage: 1 }));
