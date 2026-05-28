// src/engine/pipeline/apply.rs
//
// Execution loop for the pipeline. `apply_ops_tracked` materializes a
// `Cow<DynamicImage>` only when an operation actually requires owned
// pixels, then walks the (validated + optimized) operation list and
// returns the final image alongside a transition-updated `ColorState`.
// `apply_ops` is the legacy entry point that discards color state.
//
// Copy-on-Write logging note:
// Use DynamicImage::width()/height() (built-in methods) to avoid pulling in
// GenericImageView. If you ever need dimensions via the trait, import it
// locally to prevent duplicate/global imports.

use super::capabilities::validate_operation_sequence;
use super::color_state::update_color_state;
use super::optimize::optimize_ops;
use super::{ColorState, ColorTrackedImage, IccState, PipelineResult};
use crate::engine::resize::{
    calc_cover_resize_dimensions, calc_resize_dimensions, crop_to_dimensions,
    default_resize_options, fast_resize_owned, fast_resize_owned_impl, validate_resize_dimensions,
};
use crate::error::LazyImageError;
use crate::ops::{Operation, ResizeFit};
use image::DynamicImage;
use std::borrow::Cow;

#[cfg(feature = "cow-debug")]
use once_cell::sync::Lazy;
#[cfg(feature = "cow-debug")]
use tracing::debug;

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
