// src/engine/pipeline/tests.rs
//
// Test module for `engine::pipeline`. Mounted from `mod.rs` via
// `#[cfg(test)] #[path = "tests.rs"] mod tests;`. The original tests were
// organized as four sibling submodules; we keep that grouping here.
// `apply_ops_tests` is large enough to live in its own file (referenced
// below via `#[path]`) so each file stays under the 800-line ceiling.

use super::capabilities::{validate_operation_sequence_with_caps, OperationCapabilities};
use super::*;
use crate::engine::test_support::create_test_image;
use crate::ops::{Operation, ResizeFit};
use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};
use std::borrow::Cow;

#[path = "tests/apply_ops.rs"]
mod apply_ops_tests;

mod color_state_tests {
    use super::super::color_state::update_color_state;
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
        let img =
            DynamicImage::ImageRgb16(image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::new(2, 2));
        let mut state = ColorState::from_dynamic_image(&img, IccState::Absent);
        assert_eq!(state.bit_depth, BitDepth::Sixteen);
        state = update_color_state(state, &Operation::Grayscale);
        assert_eq!(state.color_space, ColorSpace::Luma);
        assert_eq!(state.bit_depth, BitDepth::Eight);
    }

    #[test]
    fn update_color_state_normalizes_colorspace_and_bitdepth() {
        let img =
            DynamicImage::ImageLuma16(image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(2, 2));
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
    use super::super::capabilities::validate_operation_sequence;
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
            Operation::Rotate(crate::ops::RotationAngle::Cw90),
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
