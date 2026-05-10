// src/engine/pipeline.rs
//
// Pipeline operations: apply_ops, optimize_ops, operation validation and dispatch.
// Resize logic is delegated to engine/resize.rs.

#![cfg_attr(not(feature = "napi"), allow(dead_code))]

use crate::engine::resize::{
    calc_cover_resize_dimensions, calc_resize_dimensions, crop_to_dimensions,
    default_resize_options, fast_resize_owned, fast_resize_owned_impl, validate_resize_dimensions,
};
use crate::error::LazyImageError;
use crate::ops::{Operation, OperationContract, OperationEffect, OperationRequirement, ResizeFit};
use image::DynamicImage;
use std::borrow::Cow;

#[cfg(feature = "cow-debug")]
use once_cell::sync::Lazy;
#[cfg(feature = "cow-debug")]
use tracing::debug;

// Copy-on-Write logging note:
// Use DynamicImage::width()/height() (built-in methods) to avoid pulling in GenericImageView.
// If you ever need dimensions via the trait, import it locally to prevent duplicate/global imports.

// Type alias for Result - always use LazyImageError to preserve error taxonomy
// This ensures that pipeline errors are properly classified (CodecError, UserError, etc.)
// rather than being converted to generic InternalBug errors.
type PipelineResult<T> = std::result::Result<T, LazyImageError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OperationCapabilities {
    decoded_pixels: bool,
    color_state_tracked: bool,
    orientation_available: bool,
}

impl OperationCapabilities {
    fn with_defaults() -> Self {
        Self {
            decoded_pixels: true,
            color_state_tracked: true,
            // EXIF Orientation is parsed during decode; assume available unless stripped.
            orientation_available: true,
        }
    }

    fn meets(&self, contract: &OperationContract) -> bool {
        (!contract
            .requires
            .contains(OperationRequirement::DECODED_PIXELS)
            || self.decoded_pixels)
            && (!contract
                .requires
                .contains(OperationRequirement::COLOR_STATE)
                || self.color_state_tracked)
            && (!contract
                .requires
                .contains(OperationRequirement::ORIENTATION)
                || self.orientation_available)
    }

    fn apply(&mut self, contract: &OperationContract) {
        if contract.effects.contains(OperationEffect::NORMALIZES_COLOR) {
            self.color_state_tracked = true;
        }
    }
}

fn validate_operation_sequence(ops: &[Operation]) -> PipelineResult<()> {
    let mut caps = OperationCapabilities::with_defaults();
    validate_operation_sequence_with_caps(ops, &mut caps)
}

fn validate_operation_sequence_with_caps(
    ops: &[Operation],
    caps: &mut OperationCapabilities,
) -> PipelineResult<()> {
    for op in ops {
        let contract = op.contract();
        if !caps.meets(&contract) {
            return Err(LazyImageError::invalid_argument(
                "operation",
                contract.name,
                "operation prerequisites are not satisfied (missing required state)",
            ));
        }
        caps.apply(&contract);
    }
    Ok(())
}

fn update_color_state(mut state: ColorState, op: &Operation) -> ColorState {
    match op {
        Operation::Grayscale => {
            // Grayscale converts any input to luma (alpha is stripped).
            state.color_space = ColorSpace::Luma;
            // Pipeline uses to_luma8(), so bit depth is always 8-bit after this op.
            state.bit_depth = BitDepth::Eight;
        }
        Operation::Brightness { .. } | Operation::Contrast { .. } => {
            // These ops operate on 8-bit buffers in our pipeline; if we had 16-bit, mark it unknown.
            if state.bit_depth == BitDepth::Sixteen {
                state.bit_depth = BitDepth::Unknown;
            }
        }
        Operation::ColorSpace { target: _ } => {
            // Pixel-format normalization forces RGB8 (no alpha) today.
            state.color_space = ColorSpace::Rgb;
            state.bit_depth = BitDepth::Eight;
            state.transfer = TransferFn::Srgb;
        }
        Operation::Resize { .. }
        | Operation::Extract { .. }
        | Operation::Crop { .. }
        | Operation::Rotate { .. }
        | Operation::FlipH
        | Operation::FlipV
        | Operation::AutoOrient { .. } => {}
    }
    state
}

/// Color representation tracked through the pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpace {
    Rgb,
    Rgba,
    Luma,
    LumaA,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitDepth {
    Eight,
    Sixteen,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferFn {
    Srgb,
    Unknown,
}

/// Presence of ICC profile.
#[cfg_attr(not(feature = "napi"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IccState {
    Present,
    Absent,
}

/// Pipeline color state (color space + bit depth + transfer + ICC presence).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorState {
    pub color_space: ColorSpace,
    pub bit_depth: BitDepth,
    pub transfer: TransferFn,
    pub icc: IccState,
}

impl ColorState {
    pub fn from_dynamic_image(img: &DynamicImage, icc: IccState) -> Self {
        let color_space = match img {
            DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgb16(_) => ColorSpace::Rgb,
            DynamicImage::ImageRgba8(_) | DynamicImage::ImageRgba16(_) => ColorSpace::Rgba,
            DynamicImage::ImageLuma8(_) | DynamicImage::ImageLuma16(_) => ColorSpace::Luma,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageLumaA16(_) => ColorSpace::LumaA,
            _ => ColorSpace::Unknown,
        };
        let bit_depth = match img {
            DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
            | DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_) => BitDepth::Eight,
            DynamicImage::ImageRgb16(_)
            | DynamicImage::ImageRgba16(_)
            | DynamicImage::ImageLuma16(_)
            | DynamicImage::ImageLumaA16(_) => BitDepth::Sixteen,
            _ => BitDepth::Unknown,
        };

        let transfer = if matches!(color_space, ColorSpace::Unknown) {
            TransferFn::Unknown
        } else {
            TransferFn::Srgb
        };

        Self {
            color_space,
            bit_depth,
            transfer,
            icc,
        }
    }
}

/// Image plus tracked color state.
#[cfg_attr(not(feature = "napi"), allow(dead_code))]
pub struct ColorTrackedImage<'a> {
    pub image: Cow<'a, DynamicImage>,
    pub state: ColorState,
}

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

/// Apply all queued operations using Copy-on-Write semantics
///
/// **Design Philosophy**: This function embodies the boundary between
/// "immutable engine" and "disposable task":
/// - The engine is immutable: operations are queued but not executed until `toBuffer()` is called
/// - Tasks are disposable: each `toBuffer()` call creates a new task that owns a clone of operations
/// - This function is called within a task context, where `ops` is already cloned from the engine
///
/// **True Copy-on-Write**: If no operations are queued (format conversion only),
/// returns `Cow::Borrowed` - no pixel data is copied. Deep copy only happens
/// when actual image manipulation (resize, crop, etc.) is required.
///
/// **Operation Cloning**: The `ops` parameter is cloned internally via `optimize_ops()`.
/// This is intentional and low-cost (operations are small structs), ensuring that:
/// - The original engine's operation queue remains unchanged
/// - Each task can optimize operations independently
/// - The design maintains clear separation between immutable engine state and task execution
pub fn apply_ops_tracked<'a>(
    img: Cow<'a, DynamicImage>,
    ops: &[Operation],
    initial_state: ColorState,
) -> PipelineResult<ColorTrackedImage<'a>> {
    // Optional debug logging for copy-on-write events.
    // Enabled only when feature "cow-debug" is on AND env LAZY_IMAGE_DEBUG_COW=1.
    #[cfg(feature = "cow-debug")]
    static COW_DEBUG_ENABLED: Lazy<bool> =
        Lazy::new(|| std::env::var("LAZY_IMAGE_DEBUG_COW").is_ok());
    #[cfg(feature = "cow-debug")]
    let log_copy = |stage: &str, dims: (u32, u32)| {
        if *COW_DEBUG_ENABLED {
            debug!(target: "lazy_image::cow", %stage, width = dims.0, height = dims.1, "copy-on-write");
        }
    };
    #[cfg(not(feature = "cow-debug"))]
    let log_copy = |_stage: &str, _dims: (u32, u32)| {};

    // Validate and optimize operations first
    validate_operation_sequence(ops)?;
    // Note: This clones ops internally, which is intentional for the immutable engine design.
    // The clone cost is low (ops are small structs) and ensures task isolation.
    let optimized_ops = optimize_ops(ops);

    // No operations = no copy needed (format conversion only path)
    if optimized_ops.is_empty() {
        return Ok(ColorTrackedImage {
            image: img,
            state: initial_state,
        });
    }

    // Operations exist - we need owned data to mutate
    // This is where the "copy" in Copy-on-Write happens
    log_copy(
        "into_owned (materialize for ops)",
        (img.width(), img.height()),
    );
    let mut img = img.into_owned();
    let mut state = initial_state;

    for op in &optimized_ops {
        state = update_color_state(state, op);
        img = match op {
            Operation::Resize { width, height, fit } => match (fit, width, height) {
                (ResizeFit::Fill, Some(w), Some(h)) => {
                    let target_w = *w;
                    let target_h = *h;
                    validate_resize_dimensions(target_w, target_h)?;
                    if (target_w, target_h) == (img.width(), img.height()) {
                        img
                    } else {
                        let src_image = match img {
                            DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img,
                            _ => {
                                log_copy(
                                    "to_rgba8 (normalize before resize)",
                                    (img.width(), img.height()),
                                );
                                DynamicImage::ImageRgba8(img.to_rgba8())
                            }
                        };
                        fast_resize_owned(src_image, target_w, target_h)
                            .map_err(|err| err.into_lazy_image_error())?
                    }
                }
                (ResizeFit::Cover, Some(target_w), Some(target_h)) => {
                    validate_resize_dimensions(*target_w, *target_h)?;
                    if (*target_w, *target_h) == (img.width(), img.height()) {
                        img
                    } else {
                        let (resize_w, resize_h) = calc_cover_resize_dimensions(
                            img.width(),
                            img.height(),
                            *target_w,
                            *target_h,
                        );
                        let src_image = match img {
                            DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img,
                            _ => {
                                log_copy(
                                    "to_rgba8 (normalize before cover resize)",
                                    (img.width(), img.height()),
                                );
                                DynamicImage::ImageRgba8(img.to_rgba8())
                            }
                        };
                        let resized = fast_resize_owned(src_image, resize_w, resize_h)
                            .map_err(|err| err.into_lazy_image_error())?;
                        crop_to_dimensions(resized, *target_w, *target_h)
                    }
                }
                _ => {
                    let (w, h) = calc_resize_dimensions(img.width(), img.height(), *width, *height);
                    validate_resize_dimensions(w, h)?;
                    if (w, h) == (img.width(), img.height()) {
                        img
                    } else {
                        let src_image = match img {
                            DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img,
                            _ => {
                                log_copy(
                                    "to_rgba8 (normalize before resize/extract)",
                                    (img.width(), img.height()),
                                );
                                DynamicImage::ImageRgba8(img.to_rgba8())
                            }
                        };
                        fast_resize_owned(src_image, w, h)
                            .map_err(|err| err.into_lazy_image_error())?
                    }
                }
            },
            Operation::Extract {
                width,
                height,
                fit,
                crop_x,
                crop_y,
                crop_width,
                crop_height,
            } => {
                if *crop_width == 0 || *crop_height == 0 {
                    return Err(LazyImageError::invalid_crop_dimensions(
                        *crop_width,
                        *crop_height,
                    ));
                }

                // Calculate resize target identical to Resize branch
                let (resize_w, resize_h) = match (fit, width, height) {
                    (ResizeFit::Fill, Some(w), Some(h)) => (*w, *h),
                    (ResizeFit::Cover, Some(target_w), Some(target_h)) => {
                        calc_cover_resize_dimensions(
                            img.width(),
                            img.height(),
                            *target_w,
                            *target_h,
                        )
                    }
                    _ => calc_resize_dimensions(img.width(), img.height(), *width, *height),
                };

                validate_resize_dimensions(resize_w, resize_h)?;

                let (frame_w, frame_h, offset_x, offset_y) = match (fit, width, height) {
                    (ResizeFit::Cover, Some(target_w), Some(target_h)) => {
                        let off_x = (resize_w.saturating_sub(*target_w)) / 2;
                        let off_y = (resize_h.saturating_sub(*target_h)) / 2;
                        (*target_w, *target_h, off_x, off_y)
                    }
                    _ => (resize_w, resize_h, 0, 0),
                };

                if *crop_x + *crop_width > frame_w || *crop_y + *crop_height > frame_h {
                    return Err(LazyImageError::invalid_crop_bounds(
                        *crop_x,
                        *crop_y,
                        *crop_width,
                        *crop_height,
                        frame_w,
                        frame_h,
                    ));
                }

                if (resize_w, resize_h) == (img.width(), img.height())
                    && *crop_x == 0
                    && *crop_y == 0
                    && *crop_width == resize_w
                    && *crop_height == resize_h
                {
                    // Degenerate case: no-op extract
                    img
                } else {
                    // Map crop region back to source coordinates to avoid resizing unused areas.
                    let scale_x = resize_w as f64 / img.width().max(1) as f64;
                    let scale_y = resize_h as f64 / img.height().max(1) as f64;

                    let src_left = (offset_x as f64 + *crop_x as f64) / scale_x;
                    let src_top = (offset_y as f64 + *crop_y as f64) / scale_y;
                    let mut src_width = *crop_width as f64 / scale_x;
                    let mut src_height = *crop_height as f64 / scale_y;
                    let max_src_width = img.width().max(1) as f64 - src_left;
                    let max_src_height = img.height().max(1) as f64 - src_top;
                    if src_width > max_src_width {
                        src_width = max_src_width;
                    }
                    if src_height > max_src_height {
                        src_height = max_src_height;
                    }

                    let src_image = match img {
                        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img,
                        _ => {
                            log_copy(
                                "to_rgba8 (normalize before crop->resize path)",
                                (img.width(), img.height()),
                            );
                            DynamicImage::ImageRgba8(img.to_rgba8())
                        }
                    };

                    fast_resize_owned_impl(
                        src_image,
                        *crop_width,
                        *crop_height,
                        default_resize_options().crop(src_left, src_top, src_width, src_height),
                    )
                    .map_err(|err| err.into_lazy_image_error())?
                }
            }

            Operation::Crop {
                x,
                y,
                width,
                height,
            } => {
                if *width == 0 || *height == 0 {
                    return Err(LazyImageError::invalid_crop_dimensions(*width, *height));
                }
                // Validate crop bounds
                let img_w = img.width();
                let img_h = img.height();
                if *x + *width > img_w || *y + *height > img_h {
                    return Err(LazyImageError::invalid_crop_bounds(
                        *x, *y, *width, *height, img_w, img_h,
                    ));
                }
                img.crop_imm(*x, *y, *width, *height)
            }

            Operation::Rotate { degrees } => {
                match degrees {
                    90 => img.rotate90(),
                    180 => img.rotate180(),
                    270 => img.rotate270(),
                    -90 => img.rotate270(),
                    -180 => img.rotate180(),
                    -270 => img.rotate90(),
                    0 => img, // No-op for 0 degrees
                    _ => {
                        return Err(LazyImageError::invalid_rotation_angle(*degrees));
                    }
                }
            }

            Operation::FlipH => img.fliph(),
            Operation::FlipV => img.flipv(),
            Operation::Grayscale => DynamicImage::ImageLuma8(img.to_luma8()),

            Operation::Brightness { value } => img.brighten(*value),

            Operation::Contrast { value } => {
                // image crate expects f32, convert from our -100..100 scale
                img.adjust_contrast(*value as f32)
            }

            Operation::AutoOrient { orientation } => {
                match orientation {
                    1 => img,
                    2 => img.fliph(),
                    3 => img.rotate180(),
                    4 => img.flipv(),
                    5 => img.rotate90().fliph(), // transpose
                    6 => img.rotate90(),
                    7 => img.rotate270().fliph(), // transverse
                    8 => img.rotate270(),
                    _ => img, // Ignore invalid values silently
                }
            }

            Operation::ColorSpace {
                target: crate::ops::ColorSpace::Srgb,
            } => {
                // Ensure RGB8/RGBA8 format (pixel format normalization, not color space conversion)
                match img {
                    DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img,
                    _ => DynamicImage::ImageRgb8(img.to_rgb8()),
                }
            }
        };
    }
    Ok(ColorTrackedImage {
        image: Cow::Owned(img),
        state,
    })
}

/// Backward-compatible wrapper that drops color state.
pub fn apply_ops<'a>(
    img: Cow<'a, DynamicImage>,
    ops: &[Operation],
) -> PipelineResult<Cow<'a, DynamicImage>> {
    let init_state = ColorState::from_dynamic_image(img.as_ref(), IccState::Absent);
    Ok(apply_ops_tracked(img, ops, init_state)?.image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::resize::{
        calc_cover_resize_dimensions, crop_to_dimensions, fast_resize_owned,
    };
    use crate::ops::{Operation, ResizeFit};
    use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};
    use std::borrow::Cow;

    // Helper function to create test images
    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }))
    }

    mod color_state_tests {
        use super::*;

        #[test]
        fn color_state_from_dynamic_image_detects_rgba_8bit() {
            let img = DynamicImage::ImageRgba8(RgbaImage::new(2, 2));
            let state = ColorState::from_dynamic_image(&img, IccState::Present);
            assert_eq!(state.color_space, ColorSpace::Rgba);
            assert_eq!(state.bit_depth, BitDepth::Eight);
            assert_eq!(state.icc, IccState::Present);
            assert_eq!(state.transfer, TransferFn::Srgb);
        }

        #[test]
        fn update_color_state_sets_luma_on_grayscale() {
            let img = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
            let mut state = ColorState::from_dynamic_image(&img, IccState::Absent);
            state = update_color_state(state, &Operation::Grayscale);
            assert_eq!(state.color_space, ColorSpace::Luma);
            assert_eq!(state.bit_depth, BitDepth::Eight);
        }

        #[test]
        fn update_color_state_converts_bit_depth_on_grayscale_16bit() {
            let img = DynamicImage::ImageRgb16(
                image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::new(2, 2),
            );
            let mut state = ColorState::from_dynamic_image(&img, IccState::Absent);
            assert_eq!(state.bit_depth, BitDepth::Sixteen);
            state = update_color_state(state, &Operation::Grayscale);
            assert_eq!(state.color_space, ColorSpace::Luma);
            assert_eq!(state.bit_depth, BitDepth::Eight);
        }

        #[test]
        fn update_color_state_normalizes_colorspace_and_bitdepth() {
            let img = DynamicImage::ImageLuma16(
                image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(2, 2),
            );
            let mut state = ColorState::from_dynamic_image(&img, IccState::Absent);
            assert_eq!(state.color_space, ColorSpace::Luma);
            assert_eq!(state.bit_depth, BitDepth::Sixteen);

            state = update_color_state(
                state,
                &Operation::ColorSpace {
                    target: crate::ops::ColorSpace::Srgb,
                },
            );

            assert_eq!(state.color_space, ColorSpace::Rgb);
            assert_eq!(state.bit_depth, BitDepth::Eight);
        }

        #[test]
        fn apply_ops_tracked_keeps_icc_flag() {
            let img = DynamicImage::ImageRgb8(RgbImage::new(4, 4));
            let ops = vec![Operation::Resize {
                width: Some(2),
                height: Some(2),
                fit: ResizeFit::Inside,
            }];
            let init = ColorState::from_dynamic_image(&img, IccState::Present);
            let tracked = apply_ops_tracked(Cow::Owned(img), &ops, init).unwrap();
            assert_eq!(tracked.state.icc, IccState::Present);
        }
    }

    mod operation_contract_tests {
        use super::*;

        #[test]
        fn validate_sequence_allows_current_ops() {
            let ops = vec![
                Operation::Resize {
                    width: Some(200),
                    height: Some(100),
                    fit: ResizeFit::Inside,
                },
                Operation::Grayscale,
                Operation::Rotate { degrees: 90 },
            ];
            assert!(validate_operation_sequence(&ops).is_ok());
        }

        #[test]
        fn validate_sequence_fails_when_orientation_missing() {
            let ops = vec![Operation::AutoOrient { orientation: 6 }];
            let mut caps = OperationCapabilities::with_defaults();
            caps.orientation_available = false;
            let result = validate_operation_sequence_with_caps(&ops, &mut caps);
            assert!(result.is_err(), "should fail without orientation metadata");
        }
    }

    mod apply_ops_tests {
        use super::*;

        #[test]
        fn test_resize_operation() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Resize {
                width: Some(50),
                height: Some(50),
                fit: ResizeFit::Inside,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_resize_width_only() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Resize {
                width: Some(50),
                height: None,
                fit: ResizeFit::Inside,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 25));
        }

        #[test]
        fn test_resize_height_only() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Resize {
                width: None,
                height: Some(25),
                fit: ResizeFit::Inside,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 25));
        }

        #[test]
        fn test_resize_cover_crops_to_box() {
            let img = create_test_image(200, 100);
            let ops = vec![Operation::Resize {
                width: Some(80),
                height: Some(80),
                fit: ResizeFit::Cover,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (80, 80));
        }

        #[test]
        fn test_resize_fill_ignores_aspect_ratio() {
            let img = create_test_image(200, 100);
            let ops = vec![Operation::Resize {
                width: Some(40),
                height: Some(90),
                fit: ResizeFit::Fill,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (40, 90));
        }

        #[test]
        fn test_crop_valid() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 10,
                y: 10,
                width: 50,
                height: 50,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_crop_out_of_bounds() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 60,
                y: 60,
                width: 50,
                height: 50,
            }];
            let result = apply_ops(Cow::Owned(img), &ops);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Crop bounds"));
        }

        #[test]
        fn test_crop_at_origin() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 0,
                y: 0,
                width: 50,
                height: 50,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_crop_entire_image() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_extract_matches_resize_then_crop() {
            let img = create_test_image(120, 80);
            let ops = vec![
                Operation::Resize {
                    width: Some(60),
                    height: Some(60),
                    fit: ResizeFit::Inside,
                },
                Operation::Crop {
                    x: 5,
                    y: 10,
                    width: 30,
                    height: 20,
                },
            ];

            // Fused path via optimize_ops inside apply_ops
            let fused = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            // Expected result using explicit two-step processing (reference behavior)
            let (resize_w, resize_h) = calc_resize_dimensions(120, 80, Some(60), Some(60));
            let resized = fast_resize_owned(img, resize_w, resize_h).unwrap();
            let expected = resized.crop_imm(5, 10, 30, 20);

            assert_eq!(fused.dimensions(), (30, 20));
            assert_eq!(fused.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_extract_cover_fit_is_not_fused() {
            let img = create_test_image(160, 80); // 2:1 aspect
            let ops = vec![
                Operation::Resize {
                    width: Some(80),
                    height: Some(80),
                    fit: ResizeFit::Cover,
                },
                Operation::Crop {
                    x: 10,
                    y: 5,
                    width: 40,
                    height: 30,
                },
            ];

            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 2, "Cover fit should not be fused");

            let result = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();
            let (resize_w, resize_h) = calc_cover_resize_dimensions(160, 80, 80, 80);
            let resized = fast_resize_owned(img, resize_w, resize_h).unwrap();
            let centered = crop_to_dimensions(resized, 80, 80);
            let expected = centered.crop_imm(10, 5, 40, 30);

            assert_eq!(result.dimensions(), (40, 30));
            assert_eq!(result.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_extract_fill_fit_matches_two_step() {
            let img = create_test_image(60, 30);
            let ops = vec![
                Operation::Resize {
                    width: Some(90),
                    height: Some(60),
                    fit: ResizeFit::Fill,
                },
                Operation::Crop {
                    x: 20,
                    y: 10,
                    width: 30,
                    height: 20,
                },
            ];

            let fused = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            let resized = fast_resize_owned(img, 90, 60).unwrap();
            let expected = resized.crop_imm(20, 10, 30, 20);

            assert_eq!(fused.dimensions(), (30, 20));
            assert_eq!(fused.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_extract_at_boundary() {
            let img = create_test_image(100, 100);
            let ops = vec![
                Operation::Resize {
                    width: Some(100),
                    height: Some(100),
                    fit: ResizeFit::Fill,
                },
                Operation::Crop {
                    x: 90,
                    y: 90,
                    width: 10,
                    height: 10,
                },
            ];

            let fused = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            let resized = fast_resize_owned(img, 100, 100).unwrap();
            let expected = resized.crop_imm(90, 90, 10, 10);

            assert_eq!(fused.dimensions(), (10, 10));
            assert_eq!(fused.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_extract_small_image() {
            let img = create_test_image(2, 2);
            let ops = vec![
                Operation::Resize {
                    width: Some(1),
                    height: Some(1),
                    fit: ResizeFit::Inside,
                },
                Operation::Crop {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            ];

            let fused = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            let resized = fast_resize_owned(img, 1, 1).unwrap();
            let expected = resized.crop_imm(0, 0, 1, 1);

            assert_eq!(fused.dimensions(), (1, 1));
            assert_eq!(fused.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_extract_extreme_aspect_ratio() {
            let img = create_test_image(10_000, 50);
            let ops = vec![
                Operation::Resize {
                    width: Some(100),
                    height: Some(100),
                    fit: ResizeFit::Inside,
                },
                Operation::Crop {
                    x: 0,
                    y: 0,
                    width: 50,
                    height: 1,
                },
            ];

            let fused = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            let (resize_w, resize_h) = calc_resize_dimensions(10_000, 50, Some(100), Some(100));
            let resized = fast_resize_owned(img, resize_w, resize_h).unwrap();
            let expected = resized.crop_imm(0, 0, 50, 1);

            assert_eq!(fused.dimensions(), (50, 1));
            assert_eq!(fused.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_rotate_90() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 90 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_180() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 180 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50));
        }

        #[test]
        fn test_rotate_270() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 270 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_neg90() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: -90 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_0() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 0 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50));
        }

        #[test]
        fn test_rotate_invalid_angle() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Rotate { degrees: 45 }];
            let result = apply_ops(Cow::Owned(img), &ops);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Unsupported rotation angle"));
        }

        #[test]
        fn test_auto_orient_rotate_90() {
            let img = create_test_image(80, 40);
            let ops = vec![Operation::AutoOrient { orientation: 6 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (40, 80));
        }

        #[test]
        fn test_auto_orient_flip_horizontal() {
            let img = create_test_image(10, 10);
            let ops = vec![Operation::AutoOrient { orientation: 2 }];
            let result = apply_ops(Cow::Owned(img.clone()), &ops)
                .unwrap()
                .into_owned();

            let original = img.to_rgb8();
            let flipped = result.to_rgb8();
            assert_eq!(original.width(), flipped.width());
            assert_eq!(original.height(), flipped.height());
            for y in 0..original.height() {
                for x in 0..original.width() {
                    assert_eq!(
                        original.get_pixel(x, y),
                        flipped.get_pixel(original.width() - 1 - x, y)
                    );
                }
            }
        }

        #[test]
        fn test_auto_orient_identity() {
            let img = create_test_image(60, 40);
            let ops = vec![Operation::AutoOrient { orientation: 1 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (60, 40));
        }

        #[test]
        fn test_auto_orient_rotate_180() {
            let img = create_test_image(60, 40);
            let ops = vec![Operation::AutoOrient { orientation: 3 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (60, 40));
        }

        #[test]
        fn test_auto_orient_flip_vertical() {
            let img = create_test_image(60, 40);
            let ops = vec![Operation::AutoOrient { orientation: 4 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (60, 40));
        }

        #[test]
        fn test_auto_orient_transpose() {
            let img = create_test_image(80, 40);
            let ops = vec![Operation::AutoOrient { orientation: 5 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (40, 80));
        }

        #[test]
        fn test_auto_orient_transverse() {
            let img = create_test_image(80, 40);
            let ops = vec![Operation::AutoOrient { orientation: 7 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (40, 80));
        }

        #[test]
        fn test_auto_orient_rotate_270() {
            let img = create_test_image(80, 40);
            let ops = vec![Operation::AutoOrient { orientation: 8 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (40, 80));
        }

        #[test]
        fn test_auto_orient_invalid_orientation_ignored() {
            for orientation in [0, 9, 255] {
                let img = create_test_image(60, 40);
                let ops = vec![Operation::AutoOrient { orientation }];
                let result = apply_ops(Cow::Owned(img), &ops).unwrap();
                assert_eq!(
                    result.dimensions(),
                    (60, 40),
                    "orientation {orientation} should be no-op"
                );
            }
        }

        #[test]
        fn test_flip_h() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::FlipH];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_flip_v() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::FlipV];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_grayscale_reduces_channels() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Grayscale];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert!(matches!(*result, DynamicImage::ImageLuma8(_)));
        }

        #[test]
        fn test_brightness() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Brightness { value: 50 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_contrast() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Contrast { value: 50 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_colorspace_srgb() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::ColorSpace {
                target: crate::ops::ColorSpace::Srgb,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_chained_operations() {
            let img = create_test_image(200, 100);
            let ops = vec![
                Operation::Resize {
                    width: Some(100),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Rotate { degrees: 90 },
                Operation::Grayscale,
            ];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
            assert!(matches!(*result, DynamicImage::ImageLuma8(_)));
        }

        #[test]
        fn test_extract_fusion_preserves_following_operations() {
            let img = create_test_image(60, 40);
            let ops = vec![
                Operation::Resize {
                    width: Some(30),
                    height: Some(30),
                    fit: ResizeFit::Inside,
                },
                Operation::Crop {
                    x: 4,
                    y: 3,
                    width: 12,
                    height: 10,
                },
                Operation::Rotate { degrees: 90 },
            ];

            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 2, "resize+crop should fuse into extract");
            assert!(matches!(optimized[0], Operation::Extract { .. }));

            let result = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            let (resize_w, resize_h) = calc_resize_dimensions(60, 40, Some(30), Some(30));
            let resized = fast_resize_owned(img, resize_w, resize_h).unwrap();
            let cropped = resized.crop_imm(4, 3, 12, 10);
            let expected = cropped.rotate90();

            assert_eq!(result.dimensions(), expected.dimensions());
            assert_eq!(result.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_crop_then_resize_rounds_from_cropped_bounds() {
            let img = create_test_image(101, 51);
            let ops = vec![
                Operation::Crop {
                    x: 1,
                    y: 1,
                    width: 100,
                    height: 49,
                },
                Operation::Resize {
                    width: Some(50),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];

            let result = apply_ops(Cow::Owned(img.clone()), &ops).unwrap();

            let cropped = img.crop_imm(1, 1, 100, 49);
            let (expected_w, expected_h) = calc_resize_dimensions(100, 49, Some(50), None);
            let expected = fast_resize_owned(cropped, expected_w, expected_h).unwrap();

            assert_eq!(result.dimensions(), (expected_w, expected_h));
            assert_eq!(result.to_rgba8().into_raw(), expected.to_rgba8().into_raw());
        }

        #[test]
        fn test_empty_operations() {
            let img = create_test_image(100, 100);
            let ops = vec![];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }
    }

    mod optimize_ops_tests {
        use super::*;

        #[test]
        fn test_consecutive_resizes_combined() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(400),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize {
                width,
                height: _,
                fit,
            } = &optimized[0]
            {
                assert_eq!(*width, Some(400));
                assert_eq!(*fit, ResizeFit::Inside);
            } else {
                panic!("Expected Resize operation");
            }
        }

        #[test]
        fn test_non_consecutive_resizes_not_combined() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Grayscale,
                Operation::Resize {
                    width: Some(400),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 3);
        }

        #[test]
        fn test_single_operation() {
            let ops = vec![Operation::Resize {
                width: Some(100),
                height: None,
                fit: ResizeFit::Inside,
            }];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
        }

        #[test]
        fn test_empty_operations() {
            let ops = vec![];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 0);
        }

        #[test]
        fn test_resize_then_crop_is_fused_into_extract() {
            let ops = vec![
                Operation::Resize {
                    width: Some(200),
                    height: Some(150),
                    fit: ResizeFit::Inside,
                },
                Operation::Crop {
                    x: 10,
                    y: 5,
                    width: 80,
                    height: 60,
                },
            ];

            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            match &optimized[0] {
                Operation::Extract {
                    width,
                    height,
                    fit,
                    crop_x,
                    crop_y,
                    crop_width,
                    crop_height,
                } => {
                    assert_eq!(*width, Some(200));
                    assert_eq!(*height, Some(150));
                    assert_eq!(*fit, ResizeFit::Inside);
                    assert_eq!(*crop_x, 10);
                    assert_eq!(*crop_y, 5);
                    assert_eq!(*crop_width, 80);
                    assert_eq!(*crop_height, 60);
                }
                other => panic!("expected Extract, got {other:?}"),
            }
        }

        #[test]
        fn test_multiple_consecutive_resizes() {
            let ops = vec![
                Operation::Resize {
                    width: Some(1000),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(400),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize {
                width,
                height: _,
                fit,
            } = &optimized[0]
            {
                assert_eq!(*width, Some(400));
                assert_eq!(*fit, ResizeFit::Inside);
            }
        }

        #[test]
        fn test_resize_with_both_dimensions() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(400),
                    height: Some(300),
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize { width, height, fit } = &optimized[0] {
                assert_eq!(*width, Some(400));
                assert_eq!(*height, Some(300));
                assert_eq!(*fit, ResizeFit::Inside);
            }
        }
    }
}
