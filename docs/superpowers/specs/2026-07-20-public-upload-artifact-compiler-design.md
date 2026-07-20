# Public Upload Artifact Compiler Design

Status: Proposed for Issue [#768](https://github.com/albert-einshutoin/lazy-image/issues/768)

Date: 2026-07-20

## Summary

lazy-image should offer one safe, high-level path for turning an untrusted upload into a complete set of publishable web-image artifacts. The first version targets native Node.js backends and uses a built-in `publicUpload` policy, file-to-file processing, fail-closed behavior, privacy-safe metadata handling, and directory-level publication.

The orchestration layer belongs in JavaScript. It compiles a validated policy into a deterministic execution plan, delegates image work to cloned Rust `ImageEngine` instances, verifies every output, writes a manifest, and publishes the completed directory. The Rust core remains responsible for decoding, transforms, encoding, byte-budget search, metadata enforcement, and resource limits.

This is an additive API. Existing `ImageEngine`, buffer, file, batch, and Wasm APIs remain unchanged.

## Product decision

The initial winning use case is:

> Safely preprocess an untrusted image upload into privacy-safe, byte-bounded, responsive artifacts that an application can publish only after lazy-image accepts the whole set.

The design deliberately optimizes for safety, predictable integration, low Node.js heap usage, and smaller files rather than minimum latency.

## Goals

- Provide one recommended API for untrusted upload preprocessing.
- Make successful publication an explicit trust boundary.
- Prevent partial artifact sets from becoming visible.
- Re-encode every public artifact; never pass the original upload through.
- Strip EXIF, GPS, XMP, comments, and unknown ancillary metadata.
- Preserve an ICC profile only when it passes bounded validation.
- Generate responsive variants, delivery metadata, a placeholder, and a machine-readable manifest.
- Make target-byte requirements strict rather than best-effort.
- Keep artifact file data out of the V8 heap on the normal path.
- Produce stable plans, names, ordering, policy fingerprints, and manifests within a declared compiler fingerprint.
- Reuse existing lazy-image primitives and keep implementation work divisible into small PRs.

## Non-goals

- Edge or Wasm parity in the first version.
- Buffer or stream input in the first version.
- Original-upload retention or publication.
- Arbitrary metadata preservation.
- User-defined output filenames or path templates.
- Drawing, compositing, alpha flattening, filters, or animation.
- PNG output artifacts; the compiler targets JPEG, WebP, and AVIF delivery.
- Cross-platform byte-identical codec output.
- A sharp-compatible API.
- CDN upload or object-storage publication.

## Approved design decisions

1. The first-class environment is a native Node.js backend with local temporary storage.
2. The default failure policy is fail-closed. There is no partial-result success mode.
3. `publicUpload` is a built-in policy with bounded overrides, not a free-form policy object.
4. Outputs are assembled in a private sibling staging directory and published only after all checks succeed.
5. The original upload is always decoded, validated, and re-encoded; it is never part of the artifact set.
6. EXIF, GPS, XMP, comments, and unknown metadata are stripped. ICC is preserved only if safe and supported.
7. Node.js owns orchestration and transaction handling; Rust owns image processing and enforcement.

## Architecture and trust boundary

```text
untrusted input path
        |
        v
header preflight + authoritative decode/firewall checks
        |
        v
normalize publicUpload policy
        |
        v
deterministic execution plan
        |
        v
private sibling staging directory (mode 0700 where supported)
        |
        +-- cloned Rust engine -> responsive artifacts
        +-- cloned Rust engine -> placeholder
        +-- streaming hashes and output verification
        +-- stable manifest.json
        |
        v
all required checks succeeded?
        |
      yes only
        v
rename staging directory to final outputDir
        |
        v
publishable artifact set
```

Inputs and staging files are untrusted. Only a complete directory successfully committed by the compiler is publishable.

The application must provide a trusted, application-controlled output parent directory. The compiler does not defend against a separate process with write access to that parent racing or replacing paths. This explicit deployment assumption avoids overstating what portable Node.js filesystem APIs can guarantee.

## Public API

```ts
export type ArtifactFormat = 'jpeg' | 'webp' | 'avif';

export interface ArtifactByteBudget {
  width: number;
  format: ArtifactFormat;
  maxBytes: number;
}

export interface PublicUploadPolicy {
  preset: 'publicUpload';
  widths?: readonly number[];
  formats?: readonly ArtifactFormat[];
  budgets?: readonly ArtifactByteBudget[];
  placeholder?: boolean;
}

export interface CompileImageOptions {
  inputPath: string;
  outputDir: string;
  policy: PublicUploadPolicy;
  signal?: AbortSignal;
}

export declare function compileImage(
  options: CompileImageOptions,
): Promise<ImageArtifactManifest>;
```

Example:

```js
const { compileImage } = require('@alberteinshutoin/lazy-image');

const manifest = await compileImage({
  inputPath: '/private/uploads/request-123.tmp',
  outputDir: '/srv/images/product-42',
  policy: {
    preset: 'publicUpload',
    widths: [320, 640, 1280],
    formats: ['jpeg', 'webp', 'avif'],
    budgets: [{ width: 1280, format: 'jpeg', maxBytes: 180_000 }],
    placeholder: true,
  },
});
```

`compileImage()` resolves with the exact parsed content written to `manifest.json`. It performs no work after the final directory commit, so a resolved promise always identifies a complete published set.

## `publicUpload` policy

### Defaults

| Setting | Default | Reason |
| --- | --- | --- |
| widths | `[320, 640, 1280]` | Useful responsive baseline without excessive fan-out |
| formats | `['webp']` | Modern delivery, alpha-safe, and smaller than using an unconditional JPEG fallback |
| placeholder | `true` | Makes the artifact set immediately useful to modern web UIs |
| enlargement | disabled | Upscaling wastes CPU and bytes without adding source detail |
| fit | `inside` | Preserves aspect ratio and avoids implicit cropping |
| auto orientation | enabled | Public pixels match the intended display orientation |
| processing concurrency | `1` | Predictable memory usage; lazy-image favors size and safety over latency |
| target-byte policy | strict | A declared maximum is a contract, not a hint |

### Fixed safety constraints

- Supported inputs are static JPEG, PNG, and WebP images detected by content, not extension.
- Animated or indeterminate inputs are rejected. They are not silently reduced to a first frame.
- The baseline public-upload firewall limits are 32 MiB input, 40 MP decoded pixels, and a 5-second processing limit per artifact, subject to the global 100 MP and 32,768-pixel per-side limits.
- At most 8 normalized widths, 3 formats, and 24 responsive artifacts are allowed.
- Widths must be unique positive integers from 16 through 8,192 after normalization.
- Each byte budget must target an artifact in the normalized width/format matrix and be a positive integer no greater than 32 MiB.
- Output filenames are library-generated and cannot contain caller-controlled path segments.
- JPEG is rejected for a source with alpha because v1 does not define implicit flattening or a background color. Callers can request WebP or AVIF instead.
- Unknown alpha or animation state is a failed preflight, not an optimistic `false`.

Formats use the canonical order JPEG, WebP, then AVIF regardless of caller input order. A timeout is cooperative at native stage boundaries; it prevents publication after expiry but is not presented as a hard operating-system kill for a codec call already in progress.

The values are versioned policy behavior. A future weakening of a safety bound is a breaking policy change; a stricter bound requires release notes and migration guidance.

### Width normalization

1. Convert and validate widths without lossy coercion.
2. Remove duplicates.
3. Sort ascending.
4. Remove widths larger than the display-oriented source width.
5. If every requested width is larger, use the display-oriented source width once.

The normalized plan, rather than caller input order, controls artifact order and the policy fingerprint.

## Metadata and colour policy

The current `sanitize({ policy: 'strict' })` forces `MetadataPolicy::StripAll`, which conflicts with the approved requirement to preserve safe ICC colour information. The compiler must not approximate the contract by silently choosing the more permissive `lenient` policy.

Before the compiler can ship, the Rust policy model must be able to express:

- strict input byte, pixel, dimension, timeout, EXIF, and ancillary-metadata limits;
- forced removal of EXIF, GPS, XMP, comments, and unknown metadata; and
- `KeepIcc` only when the ICC payload is supported and no larger than 512 KiB.

An unsafe, malformed, or unsupported ICC profile is stripped rather than causing the original metadata to pass through. The manifest records the requested policy and whether an ICC profile was actually preserved. It must not claim a metadata type existed merely because the policy strips that type.

Orientation is applied to pixels before EXIF is removed. No orientation tag is emitted in public artifacts.

The placeholder is an intentional exception to ICC retention. It is a tiny, blurred UI hint rather than a colour-authoritative delivery artifact, so all placeholder metadata, including ICC, is stripped. Keeping a profile of up to 512 KiB in a 16-pixel placeholder would defeat the LQIP contract. Full delivery artifacts retain safe ICC profiles.

## Staging and publication transaction

1. Resolve `inputPath` at API-call time.
2. Resolve the real path of the trusted output parent.
3. Reject an existing final `outputDir`, including a symlink.
4. Create a random sibling staging directory under the same parent. Use restrictive permissions where supported.
5. Write only library-generated relative filenames into staging.
6. Generate and verify all artifacts sequentially.
7. Write `manifest.json` last inside staging using an atomic file write.
8. Re-check that the final path does not exist.
9. Rename staging to `outputDir` on the same filesystem.
10. Resolve with the manifest and perform no subsequent work.

The API does not support overwrite in v1. Applications must publish a new versioned directory and update their own pointer separately when replacing an existing asset set.

Errors and cancellation before step 9 remove the current staging directory in `finally`. A process crash can leave a staging directory, but it cannot leave a publishable partial set because the final directory name was never committed. Staging directories contain a library-owned marker so a future cleanup utility can identify them without broad deletion.

Portable Node.js does not provide a cross-platform no-replace directory rename primitive. The no-race guarantee therefore assumes the output parent is exclusively controlled by the application. This limitation must appear in API documentation.

## Execution plan and cost model

The JavaScript compiler creates an immutable normalized plan before the first encode. The plan contains source facts, ordered variants, byte budgets, placeholder work, fixed filenames, and policy hashes.

The initial executor processes variants sequentially through `engine.clone()`:

- cloning shares the Rust-owned encoded source and preserves the original engine state;
- each artifact can still incur its own decode, resize, and encode;
- CPU cost is approximately proportional to `width count x format count`;
- target-byte output performs multiple encodes during its quality search;
- AVIF can dominate total execution time;
- source hashing adds one bounded-memory streaming read;
- artifact hashing adds one bounded-memory streaming read per output;
- only the small placeholder is read into Node.js memory to produce a data URL.

The design does not claim one shared pixel decode. A later Rust optimization may execute the same immutable plan more efficiently without changing this public API or manifest contract.

## Manifest contract

`manifest.json` uses stable key insertion and array ordering. Paths always use `/` separators and are relative to `outputDir`.

```ts
export interface ImageArtifactManifest {
  schemaVersion: 1;
  compiler: {
    name: '@alberteinshutoin/lazy-image';
    version: string;
    platform: string;
    arch: string;
    fingerprint: string;
  };
  policy: {
    preset: 'publicUpload';
    fingerprint: string;
    widths: number[];
    formats: ArtifactFormat[];
    placeholder: boolean;
  };
  source: {
    sha256: string;
    bytes: number;
    detectedFormat: 'jpeg' | 'png' | 'webp';
    encodedWidth: number;
    encodedHeight: number;
    displayWidth: number;
    displayHeight: number;
    hasAlpha: boolean;
    orientation: number | null;
    orientationApplied: boolean;
  };
  metadata: {
    mode: 'privacySafe';
    exif: 'stripped';
    gps: 'stripped';
    xmp: 'stripped';
    comments: 'stripped';
    unknownAncillary: 'stripped';
    artifactIccPolicy: 'preserve-if-safe';
    placeholderIccPolicy: 'strip';
  };
  artifacts: Array<{
    id: string;
    path: string;
    format: ArtifactFormat;
    width: number;
    height: number;
    bytes: number;
    quality: number;
    targetBytes?: number;
    iccOutcome: 'preserved' | 'absent' | 'stripped-unsafe' | 'stripped-unsupported';
    sha256: string;
  }>;
  delivery: {
    srcsets: Array<{
      format: ArtifactFormat;
      value: string;
    }>;
  };
  placeholder?: {
    path: 'placeholder.webp';
    dataUrl: string;
    format: 'webp';
    width: number;
    height: number;
    bytes: number;
    iccOutcome: 'stripped-for-placeholder';
    sha256: string;
  };
  cacheKey: string;
}
```

Artifact names are `image-{width}.{extension}` and IDs are `{width}/{format}`. The placeholder is `placeholder.webp` and the manifest is `manifest.json`.

`policy.fingerprint` hashes the canonical normalized policy. `compiler.fingerprint` scopes reproducibility to the package version, native target, and codec build. `cacheKey` hashes the schema version, compiler fingerprint, source SHA-256, and policy fingerprint.

No timestamp, duration, absolute path, temporary path, hostname, process ID, or random staging name appears in the manifest. Runtime timing metrics remain operational telemetry and are not persisted in the deterministic artifact contract.

### Determinism boundary

The following are guaranteed for the same source bytes, normalized policy, compiler fingerprint, and supported runtime:

- execution-plan contents;
- filenames and artifact IDs;
- artifact and srcset ordering;
- manifest shape and serialization;
- selected quality for deterministic target-byte search; and
- cache-key construction.

The design does not promise byte-identical output across operating systems, architectures, package versions, or different codec builds. Those differences produce a different compiler fingerprint and cache key.

## Failure and error contract

`compileImage()` rejects with an `ArtifactCompilationError` and never resolves a partial manifest.

```ts
export interface ArtifactCompilationError extends Error {
  phase: 'preflight' | 'planning' | 'processing' | 'verification' | 'commit' | 'cleanup';
  artifactId?: string;
  errorCode?: string;
  category?: ErrorCategory;
  recoveryHint?: string;
  cause?: unknown;
}
```

- Existing native `errorCode`, `category`, and `recoveryHint` are preserved.
- Invalid policy fields use the existing E400/UserError contract.
- Unsupported or corrupt uploads preserve E111, E130, or E131.
- Firewall limits preserve E121, E122, or E123.
- Strict target-byte failure preserves E300 and identifies the artifact.
- Filesystem write and commit failures preserve E301.
- A verification mismatch that cannot be attributed to caller input is an InternalBug and must not be downgraded.
- Cleanup failure never hides the primary failure. It is attached as diagnostic context without exposing private absolute paths in the public message.
- Abortion stops scheduling new work, waits for active native work to settle, removes staging, and rejects with `AbortError`. It never commits output.

Artifact processing follows deterministic plan order. This makes the reported first failing artifact stable in v1.

## Output verification

Before publication, every artifact is checked against the plan:

- content magic matches the declared format;
- decoded header dimensions match the planned dimensions;
- file size matches the recorded byte count;
- strict byte budget is met;
- SHA-256 matches the manifest entry;
- artifact path is a plain file inside staging;
- the source upload was not copied into staging;
- prohibited metadata is not emitted;
- ICC behavior matches the public-upload metadata policy; and
- the manifest references every and only generated public file.

Verification must use header/container inspection where possible and avoid a second full pixel decode. Security checks performed during authoritative decode are not replaced by header preflight.

## Security invariants

- Input format is content-sniffed; filename extensions are never trusted.
- Static JPEG, PNG, and WebP are the only accepted input classes.
- Unknown animation or alpha state fails closed.
- Global decode limits remain active even if a compiler policy bug occurs.
- Policy normalization cannot disable firewall, metadata, or byte-budget strictness.
- All caller-provided paths are separated from library-generated relative artifact names.
- The output parent must be trusted and on the same filesystem as staging.
- Existing output is never intentionally overwritten.
- Original input bytes never appear in the output set or manifest.
- Manifest values do not expose private input or temporary paths.
- Hashing is streaming and bounded-memory.
- Errors do not convert InternalBug or ResourceLimit failures into success.

## Testing strategy

Implementation follows TDD. Contract tests are written before each behavior.

### Pure policy tests

- defaults normalize to the documented plan;
- widths and formats deduplicate and sort deterministically;
- enlargement is prevented;
- invalid widths, budgets, duplicate budget selectors, and unknown keys fail E400;
- caller input arrays and objects are not mutated;
- normalized policy serialization and fingerprint are stable;
- variant and srcset ordering is stable.

### Security and metadata tests

- corrupt and unsupported inputs fail before publication;
- decompression bombs and oversized inputs are rejected;
- animated or indeterminate inputs are rejected;
- EXIF orientation is applied to pixels and removed from outputs;
- EXIF, GPS, XMP, comments, and unknown ancillary metadata are absent;
- safe bounded ICC is retained for every supported output format;
- oversized, malformed, or unsupported ICC is stripped;
- alpha input plus JPEG policy fails rather than flattening implicitly;
- original input bytes and name never appear in output.

### Transaction tests

- success publishes artifacts and manifest together;
- failure on the first, middle, or final artifact leaves no final directory;
- strict target-byte miss leaves no final directory;
- existing output is not overwritten;
- symlink destination is rejected;
- cancellation leaves no final directory;
- cleanup failure preserves the primary error;
- an injected crash before rename leaves only a marked staging directory;
- no filesystem work occurs after a successful rename.

### Manifest tests

- the same source, policy, and compiler fingerprint produce identical manifest bytes;
- source and artifact hashes match streamed file hashes;
- manifest contains no absolute paths or nondeterministic fields;
- every referenced file exists and every public file is referenced;
- policy and compiler changes alter the expected fingerprints and cache key;
- Windows paths are serialized with `/` separators.

### Cost and memory tests

- file-to-file artifact generation does not return artifact buffers to V8;
- a large source stays within the existing memory guardrails;
- execution count matches the normalized width/format matrix;
- target-byte search cost is reported honestly in documentation;
- benchmark evidence is gathered before any throughput or memory claim is added.

Every implementation PR must pass `npm run build` and `npm test`. Rust policy changes also require `cargo test --no-default-features` and the existing audit checks.

## Dependency changes required before compiler implementation

Issue #700 must provide authoritative preflight facts needed by the plan:

- display orientation;
- alpha presence with an explicit unknown/failure path; and
- static-versus-animated determination for supported containers.

Its current `hasAlpha: false when unknown` proposal is not safe enough for deciding whether JPEG output is valid. Unknown must be distinguishable or rejected before format planning.

The Rust metadata/firewall model also needs a small follow-up issue for the public-upload invariant: strict resource limits plus bounded ICC-only preservation. Using the existing lenient policy would silently weaken the approved contract.

The native binding must expose enough stable build identity to construct `compiler.fingerprint`, plus an ICC outcome that distinguishes absent, preserved, unsafe, and unsupported profiles. A boolean `iccPreserved` alone cannot support an honest manifest.

## Implementation and PR sequence

1. **Preflight contract (#700 amendment)**

   Add orientation, authoritative alpha state, and animation determination with Rust and JS tests.

2. **Public-upload Rust policy**

   Add strict limits with bounded ICC-only preservation and metadata-policy tests.

3. **Policy compiler types and pure normalization**

   Add generated TypeScript types, canonical normalization, naming, serialization, and fingerprint tests.

4. **Transactional artifact executor**

   Add staging, sequential `clone()` execution, strict byte budgets, verification, manifest writing, cancellation, and commit tests.

5. **Responsive helper alignment (#697)**

   Reuse normalization/naming primitives where compatible and avoid a conflicting responsive result contract.

6. **Placeholder alignment (#698)**

   Generate the compiler placeholder and reuse the standalone helper implementation without changing manifest determinism.

7. **CLI policy entrypoint (#696)**

   Add a compiler-oriented command that accepts a policy file and emits/prints the manifest. The simpler existing optimize command may remain additive.

8. **Evidence and adoption documentation (#769 and #703)**

   Validate real-image quality, size, memory, and workflow claims before changing competitive positioning.

Each step is a short-lived branch from `main` and a PR back to `main`. No release is required to complete the implementation sequence.

## Compatibility and versioning

- `compileImage()` and its types are additive and qualify for a minor release under the project SemVer policy.
- Existing APIs do not inherit `publicUpload` defaults.
- The manifest has its own integer `schemaVersion`.
- Additive optional manifest fields can remain schema version 1; removal, rename, meaning change, or ordering-contract change requires a new schema version.
- Safety preset changes are documented user-visible behavior. Weakening a bound or changing successful outputs requires explicit compatibility review.
- Native/Wasm parity is future additive work, not implied by the first Node.js API.

## Documentation updates required with implementation

- `docs/API.md`: API, trust boundary, filesystem assumption, errors, and example.
- `docs/ARCHITECTURE.md`: policy compiler and transaction boundary.
- `docs/METADATA_SUPPORT.md`: bounded ICC-only public-upload policy.
- `docs/ERROR_CODES.md`: wrapper error behavior and any new native code if introduced.
- `docs/THREAD_MODEL.md`: sequential v1 execution and future optimization boundary.
- `docs/ADOPTION_GUIDE.md`: untrusted upload reference workflow.
- `README.md` and `README.ja.md`: concise safe-upload entrypoint after runtime verification.
- `CHANGELOG.md`: additive API entry when implementation lands.

## Design review checklist

- [x] First user and environment are explicit.
- [x] Trust boundary and failure semantics are explicit.
- [x] Public policy cannot disable safety invariants.
- [x] Metadata claims distinguish policy from observed presence.
- [x] ICC behavior no longer conflicts with the existing strict policy.
- [x] Atomic publication assumptions and limits are documented.
- [x] Determinism is scoped by compiler fingerprint.
- [x] File-to-file memory behavior and repeated-decode cost are honest.
- [x] Alpha and animation ambiguities fail closed.
- [x] Error categories are preserved rather than flattened.
- [x] Backward compatibility and schema evolution are defined.
- [x] Work can be split into independently reviewable PRs.
