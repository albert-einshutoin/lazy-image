/**
 * Upload sanitization server example.
 *
 * This example intentionally accepts a raw image request body instead of
 * multipart/form-data. It keeps the dependency set at zero while showing the
 * Image Firewall, metadata inspection, and error-category HTTP mapping.
 *
 * Run:
 *   node examples/upload-sanitize-server.mjs
 *   node examples/upload-sanitize-server.mjs --self-test
 */

import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import { fixturePath, hasSelfTestFlag, loadLazyImage } from './_load.mjs';

const {
  ImageEngine,
  ErrorCategory,
  getErrorCategory,
  inspect,
} = loadLazyImage();

const DEFAULT_MAX_BODY_BYTES = 10 * 1024 * 1024;
const DEFAULT_MAX_PIXELS = 2_000_000;

if (hasSelfTestFlag()) {
  await runSelfTest();
} else {
  const port = Number(process.env.PORT || 3000);
  const server = createServer();
  server.listen(port, () => {
    console.log(`Upload sanitizer listening on http://127.0.0.1:${port}/upload`);
  });
}

function createServer({ maxBodyBytes = DEFAULT_MAX_BODY_BYTES, maxPixels = DEFAULT_MAX_PIXELS } = {}) {
  return http.createServer(async (req, res) => {
    try {
      if (req.method !== 'POST' || req.url?.split('?')[0] !== '/upload') {
        writeJson(res, 404, { error: 'not_found' });
        return;
      }

      const body = await readRequestBody(req, maxBodyBytes);
      const output = await sanitizeUpload(body, { maxPixels });

      res.writeHead(200, {
        'content-type': 'image/webp',
        'x-input-width': String(output.meta.width),
        'x-input-height': String(output.meta.height),
      });
      res.end(output.data);
    } catch (err) {
      const status = statusForError(err);
      writeJson(res, status, {
        error: errorLabelForStatus(status),
        category: categoryName(categoryForError(err)),
        code: errorCode(err),
        message: errorMessage(err),
      });
    }
  });
}

async function sanitizeUpload(buffer, { maxPixels = DEFAULT_MAX_PIXELS } = {}) {
  // Header inspection rejects unsupported or corrupted bytes before full decode.
  const meta = inspect(buffer);

  const data = await ImageEngine.from(buffer)
    .sanitize({ policy: 'strict' })
    .limits({ maxPixels })
    .resize({ width: 1280, fit: 'inside' })
    .toBuffer('webp', 80);

  return { data, meta };
}

async function readRequestBody(req, maxBytes) {
  const chunks = [];
  let total = 0;

  for await (const chunk of req) {
    total += chunk.length;
    if (total > maxBytes) {
      const error = new Error(`Upload exceeds ${maxBytes} bytes`);
      error.category = ErrorCategory.ResourceLimit;
      throw error;
    }
    chunks.push(chunk);
  }

  if (total === 0) {
    const error = new Error('Empty upload body');
    error.category = ErrorCategory.UserError;
    throw error;
  }

  return Buffer.concat(chunks, total);
}

function statusForError(err) {
  const category = categoryForError(err);

  switch (category) {
    case ErrorCategory.UserError:
      return 400;
    case ErrorCategory.CodecError:
      return 415;
    case ErrorCategory.ResourceLimit:
      return 413;
    case ErrorCategory.InternalBug:
      return 500;
    default:
      return 500;
  }
}

function categoryForError(err) {
  return getErrorCategory(err) ?? (err && typeof err === 'object' ? err.category : undefined);
}

function errorMessage(err) {
  return err && typeof err === 'object' && 'message' in err ? err.message : String(err);
}

function errorCode(err) {
  return err && typeof err === 'object' && 'errorCode' in err ? err.errorCode : undefined;
}

function errorLabelForStatus(status) {
  if (status === 400) return 'invalid_request';
  if (status === 413) return 'image_too_large';
  if (status === 415) return 'unsupported_or_corrupt_image';
  return 'internal_error';
}

function categoryName(category) {
  if (category === ErrorCategory.UserError) return 'UserError';
  if (category === ErrorCategory.CodecError) return 'CodecError';
  if (category === ErrorCategory.ResourceLimit) return 'ResourceLimit';
  if (category === ErrorCategory.InternalBug) return 'InternalBug';
  return 'Unknown';
}

function writeJson(res, status, body) {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
}

async function post(port, body) {
  return fetch(`http://127.0.0.1:${port}/upload`, {
    method: 'POST',
    headers: { 'content-type': 'application/octet-stream' },
    body,
  });
}

async function runSelfTest() {
  const server = createServer();
  const safetyTimer = setTimeout(() => server.close(), 30_000);
  safetyTimer.unref?.();

  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();

  try {
    const okImage = fs.readFileSync(fixturePath('test_100KB_1057x1057.jpg'));
    const okResponse = await post(port, okImage);
    assert.equal(okResponse.status, 200);
    assert((await okResponse.arrayBuffer()).byteLength > 0);

    const hugeImage = fs.readFileSync(fixturePath('test_3.2MB_5000x5000.jpg'));
    const hugeResponse = await post(port, hugeImage);
    assert.equal(hugeResponse.status, 413);

    const corruptResponse = await post(port, Buffer.from('not an image'));
    assert.equal(corruptResponse.status, 415);
  } finally {
    clearTimeout(safetyTimer);
    await new Promise((resolve) => server.close(resolve));
  }
}
