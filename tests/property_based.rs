use image::{DynamicImage, GenericImageView, RgbImage};
use lazy_image::engine::{apply_ops, calc_resize_dimensions};
use lazy_image::ops::{Operation, ResizeFit};
use proptest::prelude::*;
use std::borrow::Cow;

fn create_test_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }))
}

fn valid_crop_strategy() -> impl Strategy<Value = (u32, u32, u32, u32, u32, u32)> {
    (1u32..=64, 1u32..=64)
        .prop_flat_map(|(img_w, img_h)| {
            let crop_w = 1u32..=img_w;
            let crop_h = 1u32..=img_h;
            (Just(img_w), Just(img_h), crop_w, crop_h)
        })
        .prop_flat_map(|(img_w, img_h, crop_w, crop_h)| {
            let max_x = img_w - crop_w;
            let max_y = img_h - crop_h;
            (
                Just(img_w),
                Just(img_h),
                Just(crop_w),
                Just(crop_h),
                0u32..=max_x,
                0u32..=max_y,
            )
        })
}

fn invalid_crop_strategy() -> impl Strategy<Value = (u32, u32, u32, u32, u32, u32)> {
    (1u32..=64, 1u32..=64)
        .prop_flat_map(|(img_w, img_h)| {
            let crop_w = 1u32..=img_w;
            let crop_h = 1u32..=img_h;
            (Just(img_w), Just(img_h), crop_w, crop_h)
        })
        .prop_flat_map(|(img_w, img_h, crop_w, crop_h)| {
            let min_x = img_w - crop_w + 1;
            let min_y = img_h - crop_h + 1;
            prop_oneof![
                (
                    Just(img_w),
                    Just(img_h),
                    Just(crop_w),
                    Just(crop_h),
                    min_x..=img_w,
                    Just(0u32),
                ),
                (
                    Just(img_w),
                    Just(img_h),
                    Just(crop_w),
                    Just(crop_h),
                    Just(0u32),
                    min_y..=img_h,
                ),
            ]
        })
}

fn rotate_angle_strategy() -> impl Strategy<Value = i32> {
    prop_oneof![
        Just(0),
        Just(90),
        Just(180),
        Just(270),
        Just(-90),
        Just(-180),
        Just(-270),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_resize_inside_matches_calc(
        orig_w in 1u32..=64,
        orig_h in 1u32..=64,
        target_w in 0u32..=64,
        target_h in 0u32..=64,
    ) {
        let img = create_test_image(orig_w, orig_h);
        let (calc_w, calc_h) = calc_resize_dimensions(orig_w, orig_h, Some(target_w), Some(target_h));
        let ops = vec![Operation::Resize {
            width: Some(target_w),
            height: Some(target_h),
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);

        if calc_w == 0 || calc_h == 0 {
            prop_assert!(result.is_err());
        } else {
            let resized = result.unwrap();
            prop_assert_eq!(resized.dimensions(), (calc_w, calc_h));
            prop_assert!(calc_w <= target_w);
            prop_assert!(calc_h <= target_h);
        }
    }

    #[test]
    fn prop_resize_width_only_matches_calc(
        orig_w in 1u32..=64,
        orig_h in 1u32..=64,
        target_w in 0u32..=64,
    ) {
        let img = create_test_image(orig_w, orig_h);
        let (calc_w, calc_h) = calc_resize_dimensions(orig_w, orig_h, Some(target_w), None);
        let ops = vec![Operation::Resize {
            width: Some(target_w),
            height: None,
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);

        if calc_w == 0 || calc_h == 0 {
            prop_assert!(result.is_err());
        } else {
            let resized = result.unwrap();
            prop_assert_eq!(resized.dimensions(), (calc_w, calc_h));
            prop_assert_eq!(resized.dimensions().0, target_w);
        }
    }

    #[test]
    fn prop_resize_height_only_matches_calc(
        orig_w in 1u32..=64,
        orig_h in 1u32..=64,
        target_h in 0u32..=64,
    ) {
        let img = create_test_image(orig_w, orig_h);
        let (calc_w, calc_h) = calc_resize_dimensions(orig_w, orig_h, None, Some(target_h));
        let ops = vec![Operation::Resize {
            width: None,
            height: Some(target_h),
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);

        if calc_w == 0 || calc_h == 0 {
            prop_assert!(result.is_err());
        } else {
            let resized = result.unwrap();
            prop_assert_eq!(resized.dimensions(), (calc_w, calc_h));
            prop_assert_eq!(resized.dimensions().1, target_h);
        }
    }

    #[test]
    fn prop_crop_within_bounds_succeeds(
        params in valid_crop_strategy(),
    ) {
        let (img_w, img_h, crop_w, crop_h, x, y) = params;
        let img = create_test_image(img_w, img_h);
        let ops = vec![Operation::Crop {
            x,
            y,
            width: crop_w,
            height: crop_h,
        }];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
        prop_assert_eq!(result.dimensions(), (crop_w, crop_h));
    }

    #[test]
    fn prop_crop_out_of_bounds_errors(
        params in invalid_crop_strategy(),
    ) {
        let (img_w, img_h, crop_w, crop_h, x, y) = params;
        let img = create_test_image(img_w, img_h);
        let ops = vec![Operation::Crop {
            x,
            y,
            width: crop_w,
            height: crop_h,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_rotate_valid_angles_preserve_dimensions(
        orig_w in 1u32..=64,
        orig_h in 1u32..=64,
        degrees in rotate_angle_strategy(),
    ) {
        let img = create_test_image(orig_w, orig_h);
        let ops = vec![Operation::Rotate { degrees }];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();

        let swaps = matches!(degrees, 90 | -90 | 270 | -270);
        let expected = if swaps {
            (orig_h, orig_w)
        } else {
            (orig_w, orig_h)
        };
        prop_assert_eq!(result.dimensions(), expected);
    }

    #[test]
    fn prop_rotate_invalid_angles_error(
        orig_w in 1u32..=64,
        orig_h in 1u32..=64,
        degrees in (-360i32..=360).prop_filter("invalid rotation", |d| {
            !matches!(*d, 0 | 90 | 180 | 270 | -90 | -180 | -270)
        }),
    ) {
        let img = create_test_image(orig_w, orig_h);
        let ops = vec![Operation::Rotate { degrees }];
        let result = apply_ops(Cow::Owned(img), &ops);
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_rotate_180_commutes_with_fill_resize(
        orig_w in 1u32..=32,
        orig_h in 1u32..=32,
        target_w in 1u32..=32,
        target_h in 1u32..=32,
    ) {
        let ops_a = vec![
            Operation::Rotate { degrees: 180 },
            Operation::Resize {
                width: Some(target_w),
                height: Some(target_h),
                fit: ResizeFit::Fill,
            },
        ];
        let ops_b = vec![
            Operation::Resize {
                width: Some(target_w),
                height: Some(target_h),
                fit: ResizeFit::Fill,
            },
            Operation::Rotate { degrees: 180 },
        ];

        let out_a = apply_ops(Cow::Owned(create_test_image(orig_w, orig_h)), &ops_a).unwrap().into_owned();
        let out_b = apply_ops(Cow::Owned(create_test_image(orig_w, orig_h)), &ops_b).unwrap().into_owned();

        prop_assert_eq!(out_a.dimensions(), out_b.dimensions());
        prop_assert_eq!(out_a.to_rgba8().into_raw(), out_b.to_rgba8().into_raw());
    }
}
