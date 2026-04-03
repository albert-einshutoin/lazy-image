#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/validation.rs
//
// Input validation helpers for NAPI boundary parameters.
// Extracted from api.rs to separate validation logic from the NAPI surface.

#[cfg(feature = "napi")]
use crate::error::LazyImageError;
#[cfg(feature = "napi")]
use std::path::Path;

#[cfg(feature = "napi")]
#[derive(Debug, PartialEq, Eq)]
pub enum LimitUpdate {
    Unchanged,
    Disable,
    Set(u64),
}

#[cfg(feature = "napi")]
fn number_label(v: f64) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v.is_infinite() && v.is_sign_positive() {
        "Infinity".to_string()
    } else if v.is_infinite() && v.is_sign_negative() {
        "-Infinity".to_string()
    } else {
        v.to_string()
    }
}

#[cfg(feature = "napi")]
pub fn ensure_finite_integer(
    name: &'static str,
    value: f64,
) -> std::result::Result<i64, LazyImageError> {
    if !value.is_finite() {
        return Err(LazyImageError::invalid_argument(
            name,
            number_label(value),
            "must be a finite number",
        ));
    }
    if value.fract() != 0.0 {
        return Err(LazyImageError::invalid_argument(
            name,
            value.to_string(),
            "must be an integer",
        ));
    }
    if value.abs() > (i64::MAX as f64) {
        return Err(LazyImageError::invalid_argument(
            name,
            value.to_string(),
            "is too large",
        ));
    }
    Ok(value as i64)
}

#[cfg(feature = "napi")]
fn sanitize_dimension(
    name: &'static str,
    raw: Option<f64>,
    allow_zero: bool,
) -> std::result::Result<Option<u32>, LazyImageError> {
    match raw {
        None => Ok(None),
        Some(v) => {
            let int = ensure_finite_integer(name, v)?;
            if int < 0 {
                return Err(LazyImageError::invalid_resize_dimensions(Some(0), None));
            }
            let value_u32 = u32::try_from(int)
                .map_err(|_| LazyImageError::invalid_resize_dimensions(None, None))?;
            if !allow_zero && value_u32 == 0 {
                return Err(LazyImageError::invalid_resize_dimensions(Some(0), None));
            }
            if value_u32 > crate::engine::MAX_DIMENSION {
                return Err(LazyImageError::invalid_resize_dimensions(
                    Some(value_u32),
                    None,
                ));
            }
            Ok(Some(value_u32))
        }
    }
}

#[cfg(feature = "napi")]
pub fn sanitize_resize_dimensions(
    width: Option<f64>,
    height: Option<f64>,
) -> std::result::Result<(Option<u32>, Option<u32>), LazyImageError> {
    let width = sanitize_dimension("width", width, false)?;
    let height = sanitize_dimension("height", height, false)?;

    if width.is_none() && height.is_none() {
        return Err(LazyImageError::invalid_resize_dimensions(None, None));
    }

    Ok((width, height))
}

#[cfg(feature = "napi")]
pub fn sanitize_crop(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> std::result::Result<(u32, u32, u32, u32), LazyImageError> {
    let x_i = ensure_finite_integer("x", x)?;
    let y_i = ensure_finite_integer("y", y)?;
    let width_i = ensure_finite_integer("width", width)?;
    let height_i = ensure_finite_integer("height", height)?;

    if x_i < 0 || y_i < 0 {
        return Err(LazyImageError::invalid_argument(
            "crop offset",
            format!("x={x_i}, y={y_i}"),
            "must be zero or positive",
        ));
    }

    if width_i <= 0 || height_i <= 0 {
        let w = u32::try_from(width_i.max(0)).unwrap_or(0);
        let h = u32::try_from(height_i.max(0)).unwrap_or(0);
        return Err(LazyImageError::invalid_crop_dimensions(w, h));
    }

    let width_u32 = u32::try_from(width_i)
        .map_err(|_| LazyImageError::invalid_crop_dimensions(u32::MAX, u32::MAX))?;
    let height_u32 = u32::try_from(height_i)
        .map_err(|_| LazyImageError::invalid_crop_dimensions(u32::MAX, u32::MAX))?;

    if width_u32 > crate::engine::MAX_DIMENSION || height_u32 > crate::engine::MAX_DIMENSION {
        return Err(LazyImageError::invalid_crop_dimensions(
            width_u32, height_u32,
        ));
    }

    let x_u32 = u32::try_from(x_i)
        .map_err(|_| LazyImageError::invalid_argument("x", x_i.to_string(), "must fit into u32"))?;
    let y_u32 = u32::try_from(y_i)
        .map_err(|_| LazyImageError::invalid_argument("y", y_i.to_string(), "must fit into u32"))?;

    Ok((x_u32, y_u32, width_u32, height_u32))
}

#[cfg(feature = "napi")]
pub fn sanitize_quality(quality: Option<f64>) -> std::result::Result<Option<u8>, LazyImageError> {
    match quality {
        None => Ok(None),
        Some(v) => {
            let int = ensure_finite_integer("quality", v)?;
            if !(1..=100).contains(&int) {
                return Err(LazyImageError::invalid_argument(
                    "quality",
                    int.to_string(),
                    "must be 1-100",
                ));
            }
            let value = u8::try_from(int).map_err(|_| {
                LazyImageError::invalid_argument("quality", int.to_string(), "must be 1-100")
            })?;
            Ok(Some(value))
        }
    }
}

#[cfg(feature = "napi")]
pub fn sanitize_concurrency(concurrency: Option<f64>) -> std::result::Result<u32, LazyImageError> {
    match concurrency {
        None => Ok(0),
        Some(v) => {
            let int = ensure_finite_integer("concurrency", v)?;
            if int < 0 {
                return Err(LazyImageError::invalid_argument(
                    "concurrency",
                    int.to_string(),
                    "must be >= 0",
                ));
            }
            let value = u32::try_from(int).map_err(|_| {
                LazyImageError::invalid_argument(
                    "concurrency",
                    int.to_string(),
                    "must be within u32 range",
                )
            })?;

            if value > crate::engine::MAX_CONCURRENCY as u32 {
                return Err(LazyImageError::invalid_argument(
                    "concurrency",
                    value.to_string(),
                    format!("must be 0 or 1-{}", crate::engine::MAX_CONCURRENCY),
                ));
            }

            Ok(value)
        }
    }
}

#[cfg(feature = "napi")]
pub fn sanitize_limit(
    name: &'static str,
    value: Option<f64>,
) -> std::result::Result<LimitUpdate, LazyImageError> {
    match value {
        None => Ok(LimitUpdate::Unchanged),
        Some(v) => {
            let int = ensure_finite_integer(name, v)?;
            if int < 0 {
                return Err(LazyImageError::invalid_argument(
                    name,
                    int.to_string(),
                    "must be >= 0",
                ));
            }
            if int == 0 {
                return Ok(LimitUpdate::Disable);
            }
            let value = u64::try_from(int).map_err(|_| {
                LazyImageError::invalid_argument(name, int.to_string(), "must fit within u64")
            })?;
            Ok(LimitUpdate::Set(value))
        }
    }
}

#[cfg(feature = "napi")]
pub fn validate_output_path(path: &str) -> std::result::Result<(), LazyImageError> {
    if path.trim().is_empty() {
        return Err(LazyImageError::invalid_argument(
            "path",
            "<empty>",
            "output path must not be empty",
        ));
    }

    let path_buf = Path::new(path);
    let parent = path_buf.parent().ok_or_else(|| {
        LazyImageError::invalid_argument(
            "path",
            path.to_string(),
            "must include a parent directory",
        )
    })?;

    if !parent.exists() {
        return Err(LazyImageError::invalid_argument(
            "path",
            path.to_string(),
            "parent directory does not exist",
        ));
    }

    Ok(())
}
