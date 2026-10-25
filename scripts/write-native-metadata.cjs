// Only call after a successful build of this source, or for that build's
// verified artifact. The sidecar prevents accidentally reusing a stale file.
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const pkg = require('../package.json');

function writeMetadata(file) {
  const sha256 = createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  fs.writeFileSync(`${file}.json`, `${JSON.stringify({ version: pkg.version, sha256 }, null, 2)}\n`);
}

if (require.main === module) writeMetadata(path.resolve(process.argv[2]));
module.exports = { writeMetadata };
