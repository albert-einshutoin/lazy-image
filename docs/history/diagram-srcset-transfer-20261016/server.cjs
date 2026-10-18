const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const [directory, portText] = process.argv.slice(2);
const port = Number(portText);
if (!directory || portText === undefined || !Number.isInteger(port) || port < 0 || port > 65535) throw Error('Usage: node server.cjs <owned-preview> <port>');
const root = fs.realpathSync(directory);
const catalogBytes = fs.readFileSync(path.join(root, 'catalog.json'));
const catalog = JSON.parse(catalogBytes);
const hash = body => crypto.createHash('sha256').update(body).digest('hex');
let sequence = 0;
const server = http.createServer((request, response) => {
  const id = ++sequence, startedAt = new Date().toISOString();
  const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
  const route = catalog.routes[pathname];
  let body, representation = null, error = null;
  const headers = { 'Cache-Control': 'no-store' };
  let statusCode = 200;
  try {
    if (!['GET', 'HEAD'].includes(request.method)) { statusCode = 405; body = Buffer.from('method-not-allowed\n'); }
    else if (!route) { statusCode = 404; body = Buffer.from('not-in-catalog\n'); }
    else {
      const file = fs.realpathSync(path.join(root, route.file));
      if (!file.startsWith(root + path.sep)) throw Error('outside-owned-preview');
      body = fs.readFileSync(file);
      if (body.length !== route.bytes || hash(body) !== route.sha256) throw Error('prepared-file-changed');
      representation = { bytes: route.bytes, sha256: route.sha256 };
      headers['Content-Type'] = route.type;
      if (route.type === 'image/webp') {
        headers['Cache-Control'] = 'public, max-age=3600, immutable'; headers.ETag = `"${route.sha256}"`;
        if (request.headers['if-none-match'] === headers.ETag) { statusCode = 304; body = Buffer.alloc(0); }
      }
    }
  } catch (failure) { statusCode = 500; error = failure.code || failure.message; body = Buffer.from('preview-input-error\n'); }
  if (statusCode !== 304) headers['Content-Length'] = body.length;
  headers['Content-Type'] ||= 'text/plain; charset=utf-8';
  if (request.method === 'HEAD') body = Buffer.alloc(0);
  let recorded = false;
  const record = responseCompleted => {
    if (recorded) return; recorded = true;
    console.log(JSON.stringify({ event: 'request', id, startedAt, completedAt: new Date().toISOString(), method: request.method,
      url: request.url, trialGroup: route?.group || null, status: statusCode, responseHeaders: headers,
      requestHeaders: Object.fromEntries(['cache-control', 'pragma', 'if-none-match', 'if-modified-since', 'sec-fetch-dest'].map(name => [name, request.headers[name] ?? null])),
      bodyBytes: body.length, sha256: hash(body), representation, responseCompleted, requestAborted: request.aborted, error,
      scope: 'finish means handed to OS, not proof of client response-body hash' }));
  };
  response.on('finish', () => record(true)); response.on('close', () => record(response.writableFinished));
  response.writeHead(statusCode, headers); response.end(body);
});
server.on('error', error => { console.error(JSON.stringify({ event: 'server-error', code: error.code })); process.exitCode = 1; });
server.listen(port, '127.0.0.1', () => console.log(JSON.stringify({ event: 'listening', host: '127.0.0.1', port: server.address().port, catalogSha256: hash(catalogBytes) })));
process.on('SIGTERM', () => server.close(() => process.exit(0)));
