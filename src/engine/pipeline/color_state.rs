// src/engine/pipeline/color_state.rs
//
// Color/bit-depth/transfer/ICC tracking types used by the pipeline plus the
// pure `update_color_state` transition helper invoked by `apply_ops_tracked`.

use crate::ops::Operation;
use image::DynamicImage;
use std::borrow::Cow;

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

/// Compute the next `ColorState` after applying `op`.
///
/// Pure transition function — does not touch pixel data. Visible to sibling
/// modules (apply, tests) via `pub(super)`.
pub(super) fn update_color_state(mut state: ColorState, op: &Operation) -> ColorState {
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
        | Operation::Rotate(_)
        | Operation::FlipH
        | Operation::FlipV
        | Operation::AutoOrient { .. } => {}
    }
    state
}
