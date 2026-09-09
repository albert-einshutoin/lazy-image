// Pause after the real staging marker exists so signal tests do not race compilation.
const fs = require('node:fs/promises');
const path = require('node:path');
const writeFile = fs.writeFile;
fs.writeFile = async function (file, ...args) {
  const result = await writeFile.call(this, file, ...args);
  if (path.basename(file) === '.lazy-image-staging') {
    await new Promise(resolve => {
      const resume = () => { process.disconnect(); resolve(); };
      process.once('SIGINT', resume);
      process.once('SIGTERM', resume);
      // A message listener keeps fork's IPC channel referenced until the signal arrives.
      process.once('message', signal => process.emit(signal));
      process.send('staged');
    });
  }
  return result;
};
