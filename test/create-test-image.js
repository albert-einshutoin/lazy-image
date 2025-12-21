// Backward-compat entry point for fixture generation
const { main } = require('./js/helpers/create-test-image');

if (require.main === module) {
  main();
}
