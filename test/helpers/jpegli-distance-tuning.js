const DEFAULT_JPEGLI_DISTANCE_CANDIDATES = Object.freeze([
  0.8,
  1.0,
  1.2,
  1.4,
  1.8,
  2.2,
  2.8,
  3.4,
  4.2,
  5.2,
  6.4,
  8.0,
]);

const DISTANCE_EPSILON = 1e-9;

function parseDistanceCandidateList(value, name = '--distance-candidates') {
  const tokens = String(value)
    .split(',')
    .map((token) => token.trim());

  if (!tokens.length || tokens.some((token) => token.length === 0)) {
    throw new Error(`${name} must be a comma-separated list of positive numbers`);
  }

  return tokens.map((token) => {
    const parsed = Number(token);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      throw new Error(`${name} must only contain positive finite numbers`);
    }
    return parsed;
  });
}

function buildDistanceCandidates(baseDistance, requestedCandidates = DEFAULT_JPEGLI_DISTANCE_CANDIDATES) {
  const values = [...requestedCandidates, baseDistance];
  const deduped = [];

  for (const value of values) {
    const distance = Number(value);
    if (!Number.isFinite(distance) || distance <= 0) {
      throw new Error('jpegli distance candidates must be positive finite numbers');
    }

    if (!deduped.some((existing) => Math.abs(existing - distance) < DISTANCE_EPSILON)) {
      deduped.push(distance);
    }
  }

  return deduped.sort((left, right) => left - right);
}

function percentDelta(value, baseline) {
  return ((value - baseline) / Math.max(baseline, 1)) * 100;
}

function buildTuningCandidate({
  distance,
  bytes,
  encodeMs,
  ssim,
  psnr,
  targetBytes,
  targetSsim,
  targetPsnr,
}) {
  return {
    distance,
    bytes,
    encodeMs,
    ssim,
    psnr,
    delta: {
      bytesPercent: percentDelta(bytes, targetBytes),
      ssim: ssim - targetSsim,
      psnr: psnr - targetPsnr,
    },
  };
}

function chooseMatchedBytesCandidate(candidates) {
  return [...candidates].sort((left, right) => {
    const byteDelta = Math.abs(left.delta.bytesPercent) - Math.abs(right.delta.bytesPercent);
    if (byteDelta !== 0) return byteDelta;

    const ssimDelta = right.ssim - left.ssim;
    if (ssimDelta !== 0) return ssimDelta;

    return left.encodeMs - right.encodeMs;
  })[0] || null;
}

function chooseMatchedQualityCandidate(candidates) {
  return [...candidates].sort((left, right) => {
    const ssimDelta = Math.abs(left.delta.ssim) - Math.abs(right.delta.ssim);
    if (ssimDelta !== 0) return ssimDelta;

    const psnrDelta = Math.abs(left.delta.psnr) - Math.abs(right.delta.psnr);
    if (psnrDelta !== 0) return psnrDelta;

    return left.bytes - right.bytes;
  })[0] || null;
}

module.exports = {
  DEFAULT_JPEGLI_DISTANCE_CANDIDATES,
  buildDistanceCandidates,
  buildTuningCandidate,
  chooseMatchedBytesCandidate,
  chooseMatchedQualityCandidate,
  parseDistanceCandidateList,
};
