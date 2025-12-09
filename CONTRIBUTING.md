# Contributing to lazy-image

Thank you for your interest in contributing to lazy-image! This document provides guidelines and information for contributors.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Project Philosophy](#project-philosophy)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)

---

## Code of Conduct

This project follows a simple code of conduct:

- Be respectful and inclusive
- Focus on constructive feedback
- Help others learn and grow

---

## Project Philosophy

Before contributing, please understand lazy-image's focused scope:

### ✅ What lazy-image IS

- **Web image optimization engine** for backends, CDNs, and build pipelines
- **Web-first formats**: JPEG, WebP, AVIF, PNG
- **Basic geometric operations**: resize, crop, rotate, flip
- **Memory-efficient**: file-based I/O, minimal V8 heap usage
- **Simple, stable API**: `from → pipeline → toBuffer/toFile`

### ❌ What lazy-image is NOT

lazy-image will **not** accept features that move it toward "Photoshop in Node":

- Text rendering, drawing, canvas primitives
- Heavy filters (blur, sharpen, artistic effects)
- GIF/APNG animation or video processing
- Full feature parity with sharp/jimp
- Real-time <10ms processing at high concurrency

Please read [docs/ROADMAP.md](./docs/ROADMAP.md) for the full Feature Acceptance Rules.

---

## Getting Started

### Prerequisites

- **Node.js** 18+
- **Rust** 1.70+
- **nasm** (for mozjpeg SIMD optimization)

### Setup

```bash
# Clone the repository
git clone https://github.com/albert-einshutoin/lazy-image.git
cd lazy-image

# Install Node.js dependencies
npm install

# Build the native module
npm run build

# Run tests
npm test
```

### Platform-specific setup

**macOS:**
```bash
brew install nasm
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install nasm
```

**Windows:**
```bash
# Install NASM from https://www.nasm.us/
# Or use chocolatey: choco install nasm
```

---

## Development Workflow

### Branch Naming

Use prefixes to categorize your work:

| Prefix | Purpose | Example |
|--------|---------|---------|
| `feat-` | New features | `feat-add-blur-detection` |
| `fix-` | Bug fixes | `fix-jpeg-decode-crash` |
| `docs-` | Documentation | `docs-add-docker-guide` |
| `test-` | Test improvements | `test-add-avif-edge-cases` |
| `refactor-` | Code refactoring | `refactor-split-engine` |
| `ci-` | CI/CD changes | `ci-add-coverage-report` |

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `test`: Adding or updating tests
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `ci`: CI/CD changes
- `chore`: Other changes that don't modify src or test files

**Examples:**
```
feat(encoder): add JPEG XL output support
fix(resize): handle 1x1 images correctly
docs(readme): add Docker deployment guide
test(edge-cases): add tests for corrupted JPEG inputs
```

---

## Pull Request Process

### Before Opening a PR

1. **Check the roadmap**: Ensure your change aligns with [docs/ROADMAP.md](./docs/ROADMAP.md)
2. **Search existing issues/PRs**: Avoid duplicate work
3. **For large changes**: Open an issue first to discuss the approach

### PR Checklist

- [ ] Code compiles without warnings (`cargo build --release`)
- [ ] All tests pass (`npm test` and `cargo test`)
- [ ] New code is covered by tests
- [ ] Documentation is updated if needed
- [ ] CHANGELOG.md is updated for user-facing changes
- [ ] Commit messages follow conventional commits format

### PR Description Template

```markdown
## Summary
[Brief description of what this PR does]

## Changes
- [List of specific changes]

## Testing
- [How you tested these changes]

## Related Issues
Closes #XX
```

---

## Coding Standards

### Rust

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Add doc comments (`///`) to public functions

```rust
/// Encode image to JPEG format with mozjpeg.
///
/// # Arguments
/// * `img` - The image to encode
/// * `quality` - Quality level (1-100)
/// * `icc` - Optional ICC profile to embed
///
/// # Returns
/// Encoded JPEG bytes
pub fn encode_jpeg(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> Result<Vec<u8>> {
    // ...
}
```

### JavaScript/TypeScript

- Use the existing code style (no additional linter configured)
- Maintain TypeScript type definitions in `index.d.ts`
- Add JSDoc comments for documentation

---

## Testing

### Running Tests

```bash
# Run all tests
npm test

# Run Rust unit tests
cargo test

# Run specific test file
node test/basic.test.js
node test/edge-cases.test.js
```

### Writing Tests

- **Unit tests**: Place in Rust `#[cfg(test)]` modules or `tests/` directory
- **Integration tests**: Place in `test/*.test.js`
- **Edge case tests**: Add to `test/edge-cases.test.js` or `tests/edge_cases.rs`

### Test Guidelines

1. Test both success and failure cases
2. Test boundary conditions (1x1 images, max dimensions, etc.)
3. Test error messages are helpful
4. Keep tests focused and isolated

---

## Documentation

### What to Document

- **New features**: Update README.md and add JSDoc/Rust doc comments
- **API changes**: Update `index.d.ts` with detailed JSDoc
- **Breaking changes**: Add migration notes to CHANGELOG.md
- **Architecture decisions**: Create ADR in `docs/ADR-XXX-*.md`

### Documentation Style

- Use clear, concise language
- Include code examples where helpful
- Keep examples runnable and tested

---

## Questions?

- **Bugs**: [Open an issue](https://github.com/albert-einshutoin/lazy-image/issues/new)
- **Features**: Check [ROADMAP.md](./docs/ROADMAP.md) first, then open a discussion
- **General questions**: Open a [GitHub Discussion](https://github.com/albert-einshutoin/lazy-image/discussions)

Thank you for contributing! 🦀🚀
