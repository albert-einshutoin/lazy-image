// src/engine/pipeline/optimize.rs
//
// `optimize_ops` — single-pass operation-list rewriter that merges
// consecutive resizes (last-writer-wins on dimensions) and fuses
// resize-then-crop pairs into a single `Extract` op when the fit mode
// permits, avoiding intermediate buffers.

use crate::engine::resize::calc_resize_dimensions;
use crate::ops::{Operation, ResizeFit};

/// Optimize operations by combining consecutive resize/crop operations.
///
/// Single-pass algorithm: walks through `ops` once, merging consecutive
/// resizes and fusing resize+crop pairs as it goes.  Each element is
/// visited exactly once, so the cost is O(n) in the number of operations.
pub fn optimize_ops(ops: &[Operation]) -> Vec<Operation> {
    if ops.len() < 2 {
        return ops.to_vec();
    }

    let mut result = Vec::with_capacity(ops.len());
    let mut i = 0;

    while i < ops.len() {
        // 1. Merge consecutive resizes (same fit mode) into a single resize.
        if let Operation::Resize {
            width: w1,
            height: h1,
            fit,
        } = &ops[i]
        {
            let mut final_width = *w1;
            let mut final_height = *h1;
            let fit_mode = *fit;

            // Absorb all immediately-following resizes with the same fit mode.
            while i + 1 < ops.len() {
                if let Operation::Resize {
                    width: w2,
                    height: h2,
                    fit: fit2,
                } = &ops[i + 1]
                {
                    if *fit2 != fit_mode {
                        break;
                    }
                    // Last writer wins: adopt the successor's specified dimensions.
                    if w2.is_some() && h2.is_some() {
                        final_width = *w2;
                        final_height = *h2;
                    } else if w2.is_some() {
                        final_width = *w2;
                        final_height = None;
                    } else if h2.is_some() {
                        final_width = None;
                        final_height = *h2;
                    }
                    i += 1; // consume the merged resize
                } else {
                    break;
                }
            }

            // 2. Try to fuse the (possibly merged) resize with a following crop.
            if i + 1 < ops.len() {
                if let Operation::Crop {
                    x,
                    y,
                    width: cw,
                    height: ch,
                } = &ops[i + 1]
                {
                    if fit_mode != ResizeFit::Cover {
                        // Cover fit scales to the larger dimension, maximizing
                        // intermediate buffers.  Fusing Cover into Extract
                        // doesn't reduce memory peak, so we only fuse
                        // Inside/Fill.
                        result.push(Operation::Extract {
                            width: final_width,
                            height: final_height,
                            fit: fit_mode,
                            crop_x: *x,
                            crop_y: *y,
                            crop_width: *cw,
                            crop_height: *ch,
                        });
                        i += 2; // consumed resize + crop
                        continue;
                    }
                }
            }

            result.push(Operation::Resize {
                width: final_width,
                height: final_height,
                fit: fit_mode,
            });
            i += 1;
            continue;
        }

        // 3. Crop then resize: pre-calculate final dimensions when fit=Inside.
        if let Operation::Crop {
            x,
            y,
            width: cw,
            height: ch,
        } = &ops[i]
        {
            if i + 1 < ops.len() {
                if let Operation::Resize {
                    width: rw,
                    height: rh,
                    fit,
                } = &ops[i + 1]
                {
                    if *fit == ResizeFit::Inside {
                        let (final_w, final_h) = calc_resize_dimensions(*cw, *ch, *rw, *rh);
                        result.push(Operation::Crop {
                            x: *x,
                            y: *y,
                            width: *cw,
                            height: *ch,
                        });
                        result.push(Operation::Resize {
                            width: Some(final_w),
                            height: Some(final_h),
                            fit: ResizeFit::Inside,
                        });
                        i += 2;
                        continue;
                    }
                }
            }
        }

        // 4. No optimization applies: pass through unchanged.
        result.push(ops[i].clone());
        i += 1;
    }

    result
}
