// src/engine/pipeline.rs
//
// Pipeline operations: apply_ops, optimize_ops, resize calculations

use crate::error::LazyImageError;
use crate::ops::{Operation, OperationContract, OperationEffect, OperationRequirement, ResizeFit};
use fast_image_resize::{self as fir, ImageBufferError, MulDiv, PixelType, ResizeOptions};
use image::{imageops::FilterType, DynamicImage, RgbImage, RgbaImage};
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
pub struct ColorTrackedImage<'a> {
    pub image: Cow<'a, DynamicImage>,
    pub state: ColorState,
}

// Note: to_pipeline_error is no longer needed
// because PipelineResult now always returns LazyImageError directly

#[derive(Debug)]
pub struct ResizeError {
    pub source_dims: (u32, u32),
    pub target_dims: (u32, u32),
    pub reason: String,
}

impl ResizeError {
    pub fn new(
        source_dims: (u32, u32),
        target_dims: (u32, u32),
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source_dims,
            target_dims,
            reason: reason.into(),
        }
    }

    pub fn into_lazy_image_error(self) -> LazyImageError {
        LazyImageError::resize_failed(self.source_dims, self.target_dims, self.reason)
    }
}

/// Calculate resize dimensions maintaining aspect ratio (fit = inside semantics)
pub fn calc_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: Option<u32>,
    target_h: Option<u32>,
) -> (u32, u32) {
    match (target_w, target_h) {
        (Some(w), Some(h)) => {
            // Maintain aspect ratio while fitting inside the specified dimensions
            let orig_ratio = orig_w as f64 / orig_h as f64;
            let target_ratio = w as f64 / h as f64;

            if orig_ratio > target_ratio {
                // Original image is wider → fit to width
                let ratio = w as f64 / orig_w as f64;
                (w, (orig_h as f64 * ratio).round() as u32)
            } else {
                // Original image is taller → fit to height
                let ratio = h as f64 / orig_h as f64;
                ((orig_w as f64 * ratio).round() as u32, h)
            }
        }
        (Some(w), None) => {
            let ratio = w as f64 / orig_w as f64;
            (w, (orig_h as f64 * ratio).round() as u32)
        }
        (None, Some(h)) => {
            let ratio = h as f64 / orig_h as f64;
            ((orig_w as f64 * ratio).round() as u32, h)
        }
        (None, None) => (orig_w, orig_h),
    }
}

fn validate_resize_dimensions(width: u32, height: u32) -> PipelineResult<()> {
    if width == 0 || height == 0 {
        return Err(LazyImageError::invalid_resize_dimensions(
            Some(width),
            Some(height),
        ));
    }
    Ok(())
}

fn calc_cover_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: u32,
    target_h: u32,
) -> (u32, u32) {
    if orig_w == 0 || orig_h == 0 {
        return (target_w.max(1), target_h.max(1));
    }
    let scale_w = target_w as f64 / orig_w as f64;
    let scale_h = target_h as f64 / orig_h as f64;
    let scale = scale_w.max(scale_h);
    let resize_w = ((orig_w as f64 * scale).ceil() as u32).max(1);
    let resize_h = ((orig_h as f64 * scale).ceil() as u32).max(1);
    (resize_w, resize_h)
}

fn crop_to_dimensions(img: DynamicImage, target_w: u32, target_h: u32) -> DynamicImage {
    let crop_width = target_w.min(img.width()).max(1);
    let crop_height = target_h.min(img.height()).max(1);
    let crop_x = if img.width() > crop_width {
        (img.width() - crop_width) / 2
    } else {
        0
    };
    let crop_y = if img.height() > crop_height {
        (img.height() - crop_height) / 2
    } else {
        0
    };
    img.crop_imm(crop_x, crop_y, crop_width, crop_height)
}

/// Optimize operations by combining consecutive resize/crop operations
pub fn optimize_ops(ops: &[Operation]) -> Vec<Operation> {
    if ops.len() < 2 {
        return ops.to_vec();
    }

    let mut optimized = Vec::new();
    let mut i = 0;

    while i < ops.len() {
        let current = &ops[i];

        // Try to combine consecutive resize operations
        if let Operation::Resize {
            width: w1,
            height: h1,
            fit,
        } = current
        {
            let mut final_width = *w1;
            let mut final_height = *h1;
            let fit_mode = fit.clone();
            let mut j = i + 1;

            // Combine all consecutive resize operations
            while j < ops.len() {
                if let Operation::Resize {
                    width: w2,
                    height: h2,
                    fit: fit2,
                } = &ops[j]
                {
                    if *fit2 != fit_mode {
                        break;
                    }
                    // If both dimensions are specified, use the last one
                    // Otherwise, maintain aspect ratio from the first resize
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
                    j += 1;
                } else {
                    break;
                }
            }

            if j > i + 1 {
                // Combined multiple resizes into one
                optimized.push(Operation::Resize {
                    width: final_width,
                    height: final_height,
                    fit: fit_mode,
                });
                i = j;
                continue;
            }
        }

        // Try to optimize crop + resize or resize + crop
        if i + 1 < ops.len() {
            match (&ops[i], &ops[i + 1]) {
                // Resize then crop: fuse into single Extract to avoid intermediate buffer
                (
                    Operation::Resize { width, height, fit },
                    Operation::Crop {
                        x,
                        y,
                        width: cw,
                        height: ch,
                    },
                ) if *fit != ResizeFit::Cover => {
                    // Cover fit scales to the larger dimension, maximizing intermediate buffers.
                    // Fusing Cover into Extract doesn't reduce memory peak, so we only fuse
                    // Inside/Fill to reduce peak memory and copies.
                    optimized.push(Operation::Extract {
                        width: *width,
                        height: *height,
                        fit: fit.clone(),
                        crop_x: *x,
                        crop_y: *y,
                        crop_width: *cw,
                        crop_height: *ch,
                    });
                    i += 2;
                    continue;
                }
                // Crop then resize: optimize by calculating final dimensions
                (
                    Operation::Crop {
                        x,
                        y,
                        width: cw,
                        height: ch,
                    },
                    Operation::Resize {
                        width: rw,
                        height: rh,
                        fit,
                    },
                ) => {
                    if *fit == ResizeFit::Inside {
                        let (final_w, final_h) = calc_resize_dimensions(*cw, *ch, *rw, *rh);
                        optimized.push(Operation::Crop {
                            x: *x,
                            y: *y,
                            width: *cw,
                            height: *ch,
                        });
                        optimized.push(Operation::Resize {
                            width: Some(final_w),
                            height: Some(final_h),
                            fit: ResizeFit::Inside,
                        });
                        i += 2;
                        continue;
                    }
                }
                // Resize then crop: keep both but order is already optimal
                _ => {}
            }
        }

        optimized.push(current.clone());
        i += 1;
    }

    optimized
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

/// Fast resize with owned DynamicImage (zero-copy for RGB/RGBA)
/// Returns Ok(resized) on success, Err(resize_error) on failure
pub fn fast_resize_owned(
    img: DynamicImage,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, ResizeError> {
    fast_resize_owned_impl(img, dst_width, dst_height, default_resize_options())
}

fn default_resize_options() -> ResizeOptions {
    ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3))
}

/// Fast resize with reference (for external API compatibility)
pub fn fast_resize(
    img: &DynamicImage,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, String> {
    let src_width = img.width();
    let src_height = img.height();

    if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
        return Err("invalid dimensions".to_string());
    }

    // Select pixel layout without forcing RGBA when not needed
    // Use into_raw() to avoid clone() - ownership transfer instead of copying
    let (pixel_type, src_pixels): (PixelType, Vec<u8>) = match img {
        DynamicImage::ImageRgb8(rgb) => {
            // Clone is necessary when we only have a reference
            let rgb_image = rgb.clone();
            (PixelType::U8x3, rgb_image.into_raw())
        }
        DynamicImage::ImageRgba8(rgba) => {
            // Clone is necessary when we only have a reference
            let rgba_image = rgba.clone();
            (PixelType::U8x4, rgba_image.into_raw())
        }
        _ => {
            let rgba = img.to_rgba8();
            (PixelType::U8x4, rgba.into_raw())
        }
    };

    fast_resize_internal_with_options(
        src_width,
        src_height,
        src_pixels,
        pixel_type,
        dst_width,
        dst_height,
        default_resize_options(),
    )
}

/// Internal resize implementation (shared by both owned and reference versions)
pub fn fast_resize_internal_with_options(
    src_width: u32,
    src_height: u32,
    src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, String> {
    fast_resize_internal_impl(
        src_width, src_height, src_pixels, pixel_type, dst_width, dst_height, options,
    )
}

/// Backward-compatible helper preserving the legacy signature without options.
pub fn fast_resize_internal(
    src_width: u32,
    src_height: u32,
    src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, String> {
    fast_resize_internal_with_options(
        src_width,
        src_height,
        src_pixels,
        pixel_type,
        dst_width,
        dst_height,
        default_resize_options(),
    )
}

fn fast_resize_owned_impl(
    img: DynamicImage,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, ResizeError> {
    let src_width = img.width();
    let src_height = img.height();

    if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
        return Err(ResizeError::new(
            (src_width, src_height),
            (dst_width, dst_height),
            "invalid dimensions for resize",
        ));
    }

    // Select pixel layout without forcing RGBA when not needed
    // Use into_raw() to avoid clone() - ownership transfer instead of copying
    let (pixel_type, src_pixels): (PixelType, Vec<u8>) = match img {
        DynamicImage::ImageRgb8(rgb) => {
            // Zero-copy: directly take ownership of the pixel buffer
            (PixelType::U8x3, rgb.into_raw())
        }
        DynamicImage::ImageRgba8(rgba) => {
            // Zero-copy: directly take ownership of the pixel buffer
            (PixelType::U8x4, rgba.into_raw())
        }
        other => {
            // For other formats, convert to RGBA (necessary conversion)
            let rgba = other.to_rgba8();
            (PixelType::U8x4, rgba.into_raw())
        }
    };

    fast_resize_internal_impl(
        src_width, src_height, src_pixels, pixel_type, dst_width, dst_height, options,
    )
    .map_err(|reason| ResizeError::new((src_width, src_height), (dst_width, dst_height), reason))
}

/// Decide whether alpha premultiplication is required for a given pixel layout.
#[inline]
fn requires_premultiply(pixel_type: PixelType) -> bool {
    matches!(pixel_type, PixelType::U8x4)
}

fn fast_resize_internal_impl(
    src_width: u32,
    src_height: u32,
    mut src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, String> {
    let pixel_count = (src_width as usize)
        .checked_mul(src_height as usize)
        .ok_or_else(|| "image dimensions overflow during resize".to_string())?;
    let required_bytes = pixel_count
        .checked_mul(pixel_type.size())
        .ok_or_else(|| "image buffer size overflow during resize".to_string())?;

    if src_pixels.len() < required_bytes {
        return Err(format!(
            "fir source image invalid buffer size. expected {required_bytes} bytes, got {} bytes",
            src_pixels.len()
        ));
    }

    let primary_result = match fir::images::Image::from_slice_u8(
        src_width,
        src_height,
        src_pixels.as_mut_slice(),
        pixel_type,
    ) {
        Ok(src_image) => {
            resize_with_source_image(src_image, pixel_type, dst_width, dst_height, options)
        }
        Err(ImageBufferError::InvalidBufferAlignment) => {
            let aligned_image = copy_pixels_to_aligned_image(
                src_width,
                src_height,
                pixel_type,
                &src_pixels,
                required_bytes,
            )?;
            resize_with_source_image(aligned_image, pixel_type, dst_width, dst_height, options)
        }
        Err(other) => Err(format!("fir source image error: {other:?}")),
    };

    match primary_result {
        Ok(img) => Ok(img),
        Err(err) => resize_with_image_crate_fallback(
            &src_pixels,
            src_width,
            src_height,
            pixel_type,
            dst_width,
            dst_height,
        )
        .map_err(|fallback_err| format!("{err}; image crate fallback failed: {fallback_err}")),
    }
}

fn copy_pixels_to_aligned_image(
    width: u32,
    height: u32,
    pixel_type: PixelType,
    src_pixels: &[u8],
    required_bytes: usize,
) -> std::result::Result<fir::images::Image<'static>, String> {
    let mut aligned_image = fir::images::Image::new(width, height, pixel_type);
    let aligned_buffer = aligned_image.buffer_mut();
    if aligned_buffer.len() != required_bytes {
        return Err(format!(
            "fir alignment fallback buffer mismatch. expected {required_bytes} bytes, got {} bytes",
            aligned_buffer.len()
        ));
    }
    aligned_buffer.copy_from_slice(&src_pixels[..required_bytes]);
    Ok(aligned_image)
}

fn resize_with_image_crate_fallback(
    src_pixels: &[u8],
    src_width: u32,
    src_height: u32,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, String> {
    let filter = FilterType::Lanczos3;
    match pixel_type {
        PixelType::U8x3 => {
            let rgb = RgbImage::from_raw(src_width, src_height, src_pixels.to_vec())
                .ok_or_else(|| "failed to build rgb image for fallback resize".to_string())?;
            Ok(DynamicImage::ImageRgb8(image::imageops::resize(
                &rgb, dst_width, dst_height, filter,
            )))
        }
        PixelType::U8x4 => {
            let rgba = RgbaImage::from_raw(src_width, src_height, src_pixels.to_vec())
                .ok_or_else(|| "failed to build rgba image for fallback resize".to_string())?;
            Ok(DynamicImage::ImageRgba8(image::imageops::resize(
                &rgba, dst_width, dst_height, filter,
            )))
        }
        _ => Err("fallback resize supports only U8x3/U8x4 pixel types".to_string()),
    }
}

/// Check if an RGBA image is fully opaque (all alpha values are 255)
/// For RGB images, always returns true (no alpha channel)
///
/// Only checks images ≥1MP - for smaller images, the check overhead exceeds
/// the premultiply cost (SIMD premultiply is very fast for small images)
fn is_fully_opaque(image: &fir::images::Image, pixel_type: PixelType, width: u32, height: u32) -> bool {
    if pixel_type != PixelType::U8x4 {
        return true; // RGB images have no alpha channel
    }

    // Size threshold: Only check large images (≥1MP)
    // For small images, premultiply is cheap (SIMD-optimized), skip the scan
    const THRESHOLD_PIXELS: u32 = 1_000_000; // 1 megapixel
    if (width as u64).saturating_mul(height as u64) < THRESHOLD_PIXELS as u64 {
        return false; // Assume not opaque, do premultiply (it's fast anyway)
    }

    // Check every 4th byte (alpha channel) in RGBA data
    // Conservative: if any alpha < 255, return false
    let buffer = image.buffer();
    buffer.iter().skip(3).step_by(4).all(|&alpha| alpha == 255)
}

fn resize_with_source_image<'a>(
    mut src_image: fir::images::Image<'a>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, String> {
    let mut dst_image = fir::images::Image::new(dst_width, dst_height, pixel_type);

    // Optimization: Skip premultiply/unpremultiply for fully opaque images
    // Check if all alpha values are 255 (fully opaque)
    // Only checks large images (≥1MP) where the benefit outweighs the scan cost
    let src_width = src_image.width();
    let src_height = src_image.height();
    let needs_premultiply = requires_premultiply(pixel_type)
        && !is_fully_opaque(&src_image, pixel_type, src_width, src_height);

    let mul_div = MulDiv::default();
    if needs_premultiply {
        mul_div
            .multiply_alpha_inplace(&mut src_image)
            .map_err(|e| format!("failed to premultiply alpha: {e}"))?;
    }

    let mut resizer = fir::Resizer::new();
    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| format!("fir resize error: {e:?}"))?;

    if needs_premultiply {
        mul_div
            .divide_alpha_inplace(&mut dst_image)
            .map_err(|e| format!("failed to unpremultiply alpha: {e}"))?;
    }

    let dst_pixels = dst_image.into_vec();
    match pixel_type {
        PixelType::U8x3 => {
            let rgb_image = RgbImage::from_raw(dst_width, dst_height, dst_pixels)
                .ok_or("failed to create rgb image from resized data")?;
            Ok(DynamicImage::ImageRgb8(rgb_image))
        }
        PixelType::U8x4 => {
            let rgba_image = RgbaImage::from_raw(dst_width, dst_height, dst_pixels)
                .ok_or("failed to create rgba image from resized data")?;
            Ok(DynamicImage::ImageRgba8(rgba_image))
        }
        _ => Err("unsupported pixel type after resize".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            // Simulate Luma16 input normalized to sRGB8
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

    fn create_test_image_rgba(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
        }))
    }

    mod resize_calc_tests {
        use super::*;

        #[test]
        fn test_both_dimensions_specified() {
            let (w, h) = calc_resize_dimensions(1000, 800, Some(500), Some(400));
            assert_eq!((w, h), (500, 400));
        }

        #[test]
        fn test_width_only_maintains_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, Some(500), None);
            assert_eq!(w, 500);
            assert_eq!(h, 250); // 1000:500 = 500:250
        }

        #[test]
        fn test_height_only_maintains_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, None, Some(250));
            assert_eq!(w, 500);
            assert_eq!(h, 250);
        }

        #[test]
        fn test_none_returns_original() {
            let (w, h) = calc_resize_dimensions(1000, 500, None, None);
            assert_eq!((w, h), (1000, 500));
        }

        #[test]
        fn test_rounding_behavior() {
            // Test rounding behavior with odd dimensions
            let (w, h) = calc_resize_dimensions(101, 51, Some(50), None);
            assert_eq!(w, 50);
            // 101:51 ≈ 50:25.2... → should round to 25
            assert_eq!(h, 25);
        }

        #[test]
        fn test_aspect_ratio_preservation_wide() {
            // Wide image (landscape)
            let (w, h) = calc_resize_dimensions(2000, 1000, Some(1000), None);
            assert_eq!(w, 1000);
            assert_eq!(h, 500);
        }

        #[test]
        fn test_aspect_ratio_preservation_tall() {
            // Tall image (portrait)
            let (w, h) = calc_resize_dimensions(1000, 2000, None, Some(1000));
            assert_eq!(w, 500);
            assert_eq!(h, 1000);
        }

        #[test]
        fn test_square_image() {
            let (w, h) = calc_resize_dimensions(100, 100, Some(50), None);
            assert_eq!(w, 50);
            assert_eq!(h, 50);
        }

        #[test]
        fn test_both_dimensions_wide_image_fits_inside() {
            // Resize wide image (6000×4000) to 800×600
            // Aspect ratio: 6000/4000 = 1.5 > 800/600 = 1.333...
            // → should fit to width: 800×533
            let (w, h) = calc_resize_dimensions(6000, 4000, Some(800), Some(600));
            assert_eq!(w, 800);
            assert_eq!(h, 533); // 4000 * (800/6000) = 533.33... → 533
        }

        #[test]
        fn test_both_dimensions_tall_image_fits_inside() {
            // Resize tall image (4000×6000) to 800×600
            // Aspect ratio: 4000/6000 = 0.666... < 800/600 = 1.333...
            // → should fit to height: 400×600
            let (w, h) = calc_resize_dimensions(4000, 6000, Some(800), Some(600));
            assert_eq!(w, 400); // 4000 * (600/6000) = 400
            assert_eq!(h, 600);
        }

        #[test]
        fn test_both_dimensions_same_aspect_ratio() {
            // Same aspect ratio: use specified dimensions as-is
            // 1000:500 = 2:1, 800:400 = 2:1
            let (w, h) = calc_resize_dimensions(1000, 500, Some(800), Some(400));
            assert_eq!((w, h), (800, 400));
        }
    }

    mod resize_fallback_tests {
        use super::*;

        fn generate_pixels(count: usize) -> Vec<u8> {
            (0..count).map(|i| (i % 251) as u8).collect()
        }

        #[test]
        fn image_crate_fallback_resizes_rgb() {
            let src_width = 8;
            let src_height = 4;
            let pixel_type = PixelType::U8x3;
            let src_pixels = generate_pixels((src_width * src_height) as usize * pixel_type.size());

            let resized = resize_with_image_crate_fallback(
                &src_pixels,
                src_width,
                src_height,
                pixel_type,
                4,
                2,
            )
            .expect("fallback resize should succeed for RGB");

            assert_eq!(resized.dimensions(), (4, 2));
            assert!(matches!(resized, DynamicImage::ImageRgb8(_)));
        }

        #[test]
        fn image_crate_fallback_resizes_rgba() {
            let src_width = 6;
            let src_height = 3;
            let pixel_type = PixelType::U8x4;
            let src_pixels = generate_pixels((src_width * src_height) as usize * pixel_type.size());

            let resized = resize_with_image_crate_fallback(
                &src_pixels,
                src_width,
                src_height,
                pixel_type,
                3,
                2,
            )
            .expect("fallback resize should succeed for RGBA");

            assert_eq!(resized.dimensions(), (3, 2));
            assert!(matches!(resized, DynamicImage::ImageRgba8(_)));
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
            assert_eq!(result.dimensions(), (50, 100)); // width and height swapped
        }

        #[test]
        fn test_rotate_180() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 180 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50)); // size unchanged
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
            let ops = vec![Operation::AutoOrient { orientation: 6 }]; // 6 = rotate 90 CW
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (40, 80));
        }

        #[test]
        fn test_auto_orient_flip_horizontal() {
            let img = create_test_image(10, 10);
            let ops = vec![Operation::AutoOrient { orientation: 2 }]; // mirror horizontal
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
            // After grayscale conversion, image is in Luma8 format
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
            // 200x100 → resize → 100x50 → rotate90 → 50x100
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

    mod fast_resize_tests {
        use super::*;

        #[test]
        fn test_fast_resize_downscale() {
            let img = create_test_image(200, 200);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_upscale() {
            let img = create_test_image(50, 50);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_aspect_ratio_change() {
            let img = create_test_image(200, 100);
            let result = fast_resize(&img, 100, 200);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 200));
        }

        #[test]
        fn test_fast_resize_invalid_dimensions() {
            let img = create_test_image(100, 100);
            let result = fast_resize(&img, 0, 100);
            assert!(result.is_err());
        }

        #[test]
        fn test_fast_resize_same_size() {
            let img = create_test_image(100, 100);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_rgba() {
            let img = create_test_image_rgba(100, 100);
            let result = fast_resize(&img, 50, 50);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (50, 50));
        }

        #[test]
        fn test_requires_premultiply_only_for_rgba() {
            assert!(requires_premultiply(PixelType::U8x4));
            assert!(!requires_premultiply(PixelType::U8x3));
        }

        #[test]
        fn test_fast_resize_rgba_respects_transparency_when_downscaling() {
            // Left pixel opaque red, right pixel fully transparent blue.
            // After downscale, color should stay red-dominant (premultiply prevents blue bleed).
            let mut img = DynamicImage::ImageRgba8(RgbaImage::new(2, 1));
            {
                let buf = img.as_mut_rgba8().unwrap();
                buf.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
                buf.put_pixel(1, 0, image::Rgba([0, 0, 255, 0]));
            }

            let resized = fast_resize(&img, 1, 1).expect("resize should succeed");
            let resized_rgba = resized.to_rgba8();
            let pixel = resized_rgba.get_pixel(0, 0);
            // Expect the visible color to stay red and not bleed blue from the transparent pixel.
            assert!(
                pixel[0] > 200,
                "red channel should dominate, got {}",
                pixel[0]
            );
            assert!(
                pixel[2] < 30,
                "blue channel should be minimal, got {}",
                pixel[2]
            );
            assert!(
                pixel[3] > 100,
                "alpha should remain non-zero, got {}",
                pixel[3]
            );
        }

        #[test]
        fn test_is_fully_opaque_skips_small_image_scan() {
            let width = 512;
            let height = 512; // < 1MP
            let mut pixels = vec![255u8; (width * height * 4) as usize];
            let image = fir::images::Image::from_slice_u8(
                width,
                height,
                pixels.as_mut_slice(),
                PixelType::U8x4,
            )
            .expect("valid RGBA image");

            assert!(
                !is_fully_opaque(&image, PixelType::U8x4, width, height),
                "small RGBA images should skip opacity scan"
            );
        }

        #[test]
        fn test_is_fully_opaque_scans_large_image() {
            let width = 1000;
            let height = 1000; // = 1MP
            let mut pixels = vec![255u8; (width * height * 4) as usize];
            let image = fir::images::Image::from_slice_u8(
                width,
                height,
                pixels.as_mut_slice(),
                PixelType::U8x4,
            )
            .expect("valid RGBA image");

            assert!(is_fully_opaque(&image, PixelType::U8x4, width, height));
        }

        #[test]
        fn test_is_fully_opaque_uses_wide_multiplication_for_threshold() {
            let mut pixels = vec![255u8; 4];
            let image = fir::images::Image::from_slice_u8(1, 1, pixels.as_mut_slice(), PixelType::U8x4)
                .expect("valid RGBA image");

            assert!(
                is_fully_opaque(&image, PixelType::U8x4, u32::MAX, u32::MAX),
                "overflow-safe pixel count should not force small-image fast path"
            );
        }
    }

    #[test]
    fn test_copy_pixels_to_aligned_image_preserves_data() {
        let width = 2;
        let height = 2;
        let mut src_pixels = vec![0u8; (width * height * 4) as usize];
        for (idx, byte) in src_pixels.iter_mut().enumerate() {
            *byte = idx as u8;
        }

        let image = copy_pixels_to_aligned_image(
            width,
            height,
            PixelType::U8x4,
            &src_pixels,
            src_pixels.len(),
        )
        .expect("should copy into aligned buffer");

        assert_eq!(image.buffer(), src_pixels.as_slice());
    }

    #[test]
    fn test_fast_resize_internal_impl_errors_on_short_buffer() {
        let res = fast_resize_internal_impl(
            4,
            4,
            vec![0u8; 10],
            PixelType::U8x3,
            2,
            2,
            default_resize_options(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_fast_resize_uses_rayon_pool() {
        // Ensure that fast_image_resize works correctly when executed inside a custom rayon pool.
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("failed to build test pool");

        pool.install(|| {
            let src_width = 256;
            let src_height = 256;
            let dst_width = 64;
            let dst_height = 64;

            // Simple opaque RGBA pattern
            let src_pixels: Vec<u8> = (0..(src_width * src_height))
                .flat_map(|i| {
                    let v = (i as u8).wrapping_mul(13);
                    [v, v, v, 255]
                })
                .collect();

            let result = fast_resize_internal_impl(
                src_width,
                src_height,
                src_pixels,
                PixelType::U8x4,
                dst_width,
                dst_height,
                default_resize_options(),
            );

            assert!(result.is_ok(), "resize failed inside rayon pool");
            let img = result.unwrap();
            assert_eq!(img.width(), dst_width);
            assert_eq!(img.height(), dst_height);
        });
    }
}
