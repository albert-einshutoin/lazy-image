'use strict';

function countMatchingRustTests(filter, listOutput) {
  return listOutput
    .split('\n')
    .filter(line => line.endsWith(': test'))
    .filter(line => line.slice(0, -6).includes(filter))
    .length;
}

module.exports = { countMatchingRustTests };
