const assert = require('assert');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));

async function testDeprecationWarning() {
    console.log('⚠️  Deprecation Warning Test');
    console.log('==========================\n');

    const originalWarn = console.warn;
    const capturedWarnings = [];
    console.warn = (...args) => {
        capturedWarnings.push(args.join(' '));
        return originalWarn.apply(console, args);
    };

    try {
        const testImagePath = resolveFixture('test_input.jpg');
        const engine = ImageEngine.fromPath(testImagePath);

        console.log('Calling toColorspace("srgb") to ensure warning is emitted...');
        engine.toColorspace('srgb');

        const warningFound = capturedWarnings.some(message =>
            message.includes('toColorspace() is deprecated')
        );
        assert.ok(warningFound, 'Expected toColorspace() deprecation warning');

        console.log('Verifying unsupported color space errors still behave correctly...');
        assert.throws(
            () => engine.toColorspace('p3'),
            /Color space 'p3' is not supported/,
            'Expected P3 to be rejected with helpful error'
        );

        assert.throws(
            () => engine.toColorspace('invalid'),
            /Unknown color space 'invalid'/,
            'Expected invalid color space to be rejected'
        );

        console.log('\n✅ Deprecation warning emitted and errors verified');
    } catch (error) {
        console.error('❌ Deprecation warning test failed:', error);
        process.exitCode = 1;
    } finally {
        console.warn = originalWarn;
    }
}

if (require.main === module) {
    testDeprecationWarning().catch(error => {
        console.error(error);
        process.exit(1);
    });
}

module.exports = { testDeprecationWarning };
