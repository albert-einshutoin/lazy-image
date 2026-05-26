// src/engine/pipeline/tests/apply_ops.rs
//
// `apply_ops` and `apply_ops_tracked` behavior tests, extracted from the
// original `pipeline.rs` test block. Mounted by `tests.rs` via
// `#[path = "tests/apply_ops.rs"] mod apply_ops_tests;`.

use super::*;
use crate::engine::resize::{
    calc_cover_resize_dimensions, calc_resize_dimensions, crop_to_dimensions, fast_resize_owned,
};

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
