// src/codecs/avif_safe.rs
//
// Safe abstractions for libavif FFI operations.
// This module provides RAII-based wrappers that hide raw pointers and
// eliminate unsafe blocks from the calling code.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::engine::{MAX_DIMENSION, MAX_PIXELS};
use crate::error::LazyImageError;
use libavif_sys::*;
use std::num::NonZeroU32;
use std::ptr::NonNull;
#[cfg(test)]
use std::{cell::Cell, thread_local};

#[cfg(test)]
thread_local! {
    static TRACK_DROPS: Cell<bool> = Cell::new(false);
    static LIVE_IMAGES: Cell<usize> = Cell::new(0);
    static LIVE_ENCODERS: Cell<usize> = Cell::new(0);
    static LIVE_RWDATA: Cell<usize> = Cell::new(0);
}

/// Safe wrapper for avifImage that manages its lifetime using RAII.
/// This eliminates the need for unsafe blocks when working with AVIF images.
pub struct SafeAvifImage {
    ptr: Option<NonNull<avifImage>>,
}

impl SafeAvifImage {
    fn validate_dimensions(
        width: u32,
        height: u32,
    ) -> Result<(NonZeroU32, NonZeroU32), LazyImageError> {
        let w = NonZeroU32::new(width)
            .ok_or_else(|| LazyImageError::encode_failed("avif", "Width must be greater than 0"))?;
        let h = NonZeroU32::new(height).ok_or_else(|| {
            LazyImageError::encode_failed("avif", "Height must be greater than 0")
        })?;

        if width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!(
                    "Dimensions exceed MAX_DIMENSION {} ({}x{})",
                    MAX_DIMENSION, width, height
                ),
            ));
        }

        let pixels = (width as u64)
            .checked_mul(height as u64)
            .ok_or_else(|| LazyImageError::encode_failed("avif", "Pixel count overflow"))?;

        if pixels > MAX_PIXELS {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Pixel count {} exceeds MAX_PIXELS {}", pixels, MAX_PIXELS),
            ));
        }

        Ok((w, h))
    }

    /// Create a new AVIF image with the specified dimensions and pixel format.
    ///
    /// # Arguments
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `depth` - Bit depth (typically 8 or 10)
    /// * `pixel_format` - Pixel format (e.g., AVIF_PIXEL_FORMAT_YUV420)
    ///
    /// # Returns
    /// Returns `Ok(SafeAvifImage)` on success, or an error if image creation fails.
    pub fn new(
        width: u32,
        height: u32,
        depth: u32,
        pixel_format: avifPixelFormat,
    ) -> Result<Self, LazyImageError> {
        let (_w, _h) = Self::validate_dimensions(width, height)?;
        let ptr = unsafe { avifImageCreate(width, height, depth, pixel_format) };
        let ptr = NonNull::new(ptr)
            .ok_or_else(|| LazyImageError::encode_failed("avif", "Failed to create AVIF image"))?;
        #[cfg(test)]
        TRACK_DROPS.with(|flag| {
            if flag.get() {
                LIVE_IMAGES.with(|c| c.set(c.get() + 1));
            }
        });
        Ok(Self { ptr: Some(ptr) })
    }

    /// Set color properties for the image.
    pub fn set_color_properties(
        &mut self,
        primaries: u16,
        transfer: u16,
        matrix: u16,
        yuv_range: avifRange,
    ) {
        let image = self
            .ptr
            .expect("SafeAvifImage pointer was released before configuration");
        unsafe {
            let raw = image.as_ptr();
            (*raw).colorPrimaries = primaries;
            (*raw).transferCharacteristics = transfer;
            (*raw).matrixCoefficients = matrix;
            (*raw).yuvRange = yuv_range;
        }
    }

    /// Set ICC profile for the image.
    ///
    /// # Arguments
    /// * `icc` - ICC profile data as a byte slice
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or an error if setting the profile fails.
    pub fn set_icc_profile(&mut self, icc: &[u8]) -> Result<(), LazyImageError> {
        let image = self.ptr.ok_or_else(|| {
            LazyImageError::encode_failed("avif", "AVIF image pointer was released")
        })?;
        let result = unsafe { avifImageSetProfileICC(image.as_ptr(), icc.as_ptr(), icc.len()) };
        if result != AVIF_RESULT_OK {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Failed to set ICC profile: {:?}", result),
            ));
        }
        Ok(())
    }

    /// Allocate YUV planes in the image.
    ///
    /// # Arguments
    /// * `planes` - Plane flags (e.g., AVIF_PLANES_YUV or AVIF_PLANES_A)
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or an error if allocation fails.
    pub fn allocate_planes(&mut self, planes: u32) -> Result<(), LazyImageError> {
        let image = self.ptr.ok_or_else(|| {
            LazyImageError::encode_failed("avif", "AVIF image pointer was released")
        })?;
        let result = unsafe { avifImageAllocatePlanes(image.as_ptr(), planes) };
        if result != AVIF_RESULT_OK {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Failed to allocate planes: {:?}", result),
            ));
        }
        Ok(())
    }

    /// Convert RGB to YUV using libavif's optimized conversion.
    ///
    /// # Arguments
    /// * `rgb` - RGB image structure
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or an error if conversion fails.
    pub fn rgb_to_yuv(&mut self, rgb: &avifRGBImage) -> Result<(), LazyImageError> {
        let image = self.ptr.ok_or_else(|| {
            LazyImageError::encode_failed("avif", "AVIF image pointer was released")
        })?;
        let result = unsafe { avifImageRGBToYUV(image.as_ptr(), rgb) };
        if result != AVIF_RESULT_OK {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Failed to convert RGB to YUV: {:?}", result),
            ));
        }
        Ok(())
    }

    /// Get a mutable reference to the alpha plane pointer.
    /// This is needed for copying alpha channel data.
    ///
    /// # Safety
    /// Caller must ensure that the alpha plane is allocated and exclusive access is held.
    pub unsafe fn alpha_plane_mut(&mut self) -> Result<NonNull<u8>, LazyImageError> {
        let image = self.ptr.ok_or_else(|| {
            LazyImageError::encode_failed("avif", "AVIF image pointer was released")
        })?;
        let plane_ptr = unsafe { (*image.as_ptr()).alphaPlane };
        NonNull::new(plane_ptr)
            .ok_or_else(|| LazyImageError::encode_failed("avif", "Alpha plane is not allocated"))
    }

    /// Get the alpha row bytes.
    pub fn alpha_row_bytes(&self) -> usize {
        let image = self
            .ptr
            .expect("SafeAvifImage pointer was released before querying alpha rows");
        unsafe { (*image.as_ptr()).alphaRowBytes as usize }
    }

    /// Get the raw pointer to the avifImage.
    /// This is only exposed when absolutely necessary for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is not used after the
    /// SafeAvifImage is dropped, and that it is not used concurrently.
    pub unsafe fn as_ptr(&self) -> *const avifImage {
        self.ptr
            .as_ref()
            .expect("SafeAvifImage pointer was released before FFI use")
            .as_ptr()
    }

    /// Get a mutable raw pointer to the avifImage.
    /// This is only exposed when absolutely necessary for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is not used after the
    /// SafeAvifImage is dropped, and that it is not used concurrently.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut avifImage {
        self.ptr
            .as_mut()
            .expect("SafeAvifImage pointer was released before FFI use")
            .as_ptr()
    }

    #[cfg(test)]
    pub fn take_raw_for_test(&mut self) -> *mut avifImage {
        self.ptr.take().map_or(std::ptr::null_mut(), |p| p.as_ptr())
    }
}

impl Drop for SafeAvifImage {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { avifImageDestroy(ptr.as_ptr()) };
        }
        #[cfg(test)]
        TRACK_DROPS.with(|flag| {
            if flag.get() {
                LIVE_IMAGES.with(|c| c.set(c.get().saturating_sub(1)));
            }
        });
    }
}

/// Safe wrapper for avifEncoder that manages its lifetime using RAII.
pub struct SafeAvifEncoder {
    ptr: Option<NonNull<avifEncoder>>,
}

impl SafeAvifEncoder {
    /// Create a new AVIF encoder.
    ///
    /// # Returns
    /// Returns `Ok(SafeAvifEncoder)` on success, or an error if encoder creation fails.
    pub fn new() -> Result<Self, LazyImageError> {
        let ptr = unsafe { avifEncoderCreate() };
        let ptr = NonNull::new(ptr).ok_or_else(|| {
            LazyImageError::encode_failed("avif", "Failed to create AVIF encoder")
        })?;
        #[cfg(test)]
        TRACK_DROPS.with(|flag| {
            if flag.get() {
                LIVE_ENCODERS.with(|c| c.set(c.get() + 1));
            }
        });
        Ok(Self { ptr: Some(ptr) })
    }

    /// Set encoder quality settings.
    ///
    /// # Arguments
    /// * `quality` - Quality value (0-100)
    /// * `quality_alpha` - Alpha quality value (0-100)
    /// * `speed` - Encoding speed (0-10, where 0 is slowest/best)
    /// * `max_threads` - Maximum number of threads to use
    pub fn configure(&mut self, quality: u8, quality_alpha: u8, speed: i32, max_threads: i32) {
        let encoder = self
            .ptr
            .expect("SafeAvifEncoder pointer was released before configuration");
        unsafe {
            let raw = encoder.as_ptr();
            (*raw).quality = quality as i32;
            (*raw).qualityAlpha = quality_alpha as i32;
            (*raw).speed = speed;
            (*raw).maxThreads = max_threads;
        }
    }

    /// Add an image to the encoder.
    ///
    /// # Arguments
    /// * `image` - Mutable reference to the SafeAvifImage to encode
    /// * `duration` - Duration in timescale units (1 for still images)
    /// * `add_image_flags` - Flags for adding the image
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or an error if adding the image fails.
    pub fn add_image(
        &mut self,
        image: &mut SafeAvifImage,
        duration: u64,
        add_image_flags: u32,
    ) -> Result<(), LazyImageError> {
        let encoder = self
            .ptr
            .ok_or_else(|| LazyImageError::encode_failed("avif", "AVIF encoder was released"))?;
        let result = unsafe {
            avifEncoderAddImage(
                encoder.as_ptr(),
                image.as_mut_ptr(),
                duration,
                add_image_flags,
            )
        };
        if result != AVIF_RESULT_OK {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Failed to add image to encoder: {:?}", result),
            ));
        }
        Ok(())
    }

    /// Finish encoding and write the result to the output buffer.
    ///
    /// # Arguments
    /// * `output` - Mutable reference to SafeAvifRwData to store the encoded data
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or an error if encoding fails.
    pub fn finish(&mut self, output: &mut SafeAvifRwData) -> Result<(), LazyImageError> {
        let encoder = self
            .ptr
            .ok_or_else(|| LazyImageError::encode_failed("avif", "AVIF encoder was released"))?;
        let result = unsafe { avifEncoderFinish(encoder.as_ptr(), output.as_mut_ptr()) };
        if result != AVIF_RESULT_OK {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Failed to finish encoding: {:?}", result),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn take_raw_for_test(&mut self) -> *mut avifEncoder {
        self.ptr.take().map_or(std::ptr::null_mut(), |p| p.as_ptr())
    }
}

impl Drop for SafeAvifEncoder {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { avifEncoderDestroy(ptr.as_ptr()) };
        }
        #[cfg(test)]
        TRACK_DROPS.with(|flag| {
            if flag.get() {
                LIVE_ENCODERS.with(|c| c.set(c.get().saturating_sub(1)));
            }
        });
    }
}

/// Safe wrapper for avifRWData that manages its lifetime using RAII.
pub struct SafeAvifRwData {
    data: avifRWData,
}

impl SafeAvifRwData {
    /// Create a new empty avifRWData structure.
    pub fn new() -> Self {
        #[cfg(test)]
        TRACK_DROPS.with(|flag| {
            if flag.get() {
                LIVE_RWDATA.with(|c| c.set(c.get() + 1));
            }
        });
        Self {
            data: unsafe { std::mem::zeroed() },
        }
    }

    /// Get the encoded data as a byte slice.
    ///
    /// # Returns
    /// Returns a byte slice containing the encoded AVIF data.
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            if self.data.data.is_null() || self.data.size == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(self.data.data, self.data.size)
            }
        }
    }

    /// Copy the encoded data into a `Vec<u8>`.
    ///
    /// # Returns
    /// Returns a `Vec<u8>` containing a copy of the encoded AVIF data.
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }

    /// Get a mutable raw pointer to the avifRWData.
    /// This is only exposed when absolutely necessary for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is not used after the
    /// SafeAvifRwData is dropped.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut avifRWData {
        &mut self.data
    }
}

impl Drop for SafeAvifRwData {
    fn drop(&mut self) {
        unsafe {
            avifRWDataFree(&mut self.data);
        }
        #[cfg(test)]
        TRACK_DROPS.with(|flag| {
            if flag.get() {
                LIVE_RWDATA.with(|c| c.set(c.get().saturating_sub(1)));
            }
        });
    }
}

impl Default for SafeAvifRwData {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create and configure an avifRGBImage structure.
/// This encapsulates the unsafe operations needed to set up RGB image data.
pub fn create_rgb_image(
    image: &mut SafeAvifImage,
    pixels: *const u8,
    width: u32,
    height: u32,
) -> Result<avifRGBImage, LazyImageError> {
    // Ensure dimensions are non-zero and within global bounds.
    SafeAvifImage::validate_dimensions(width, height)?;

    // rowBytes = width * 4 (RGBA8). Validate against overflow and libavif expectations (u32).
    let row_bytes_u32: u32 = width.checked_mul(4).ok_or_else(|| {
        LazyImageError::encode_failed("avif", "row bytes overflow for RGBA image")
    })?;

    let total_bytes: usize = (row_bytes_u32 as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| {
            LazyImageError::encode_failed("avif", "pixel buffer size overflow for RGBA image")
        })?;

    if total_bytes == 0 {
        return Err(LazyImageError::encode_failed(
            "avif",
            "pixel buffer size must be greater than 0",
        ));
    }

    if pixels.is_null() {
        return Err(LazyImageError::encode_failed(
            "avif",
            "pixel buffer pointer is null",
        ));
    }

    let mut rgb: avifRGBImage = unsafe { std::mem::zeroed() };
    unsafe {
        avifRGBImageSetDefaults(&mut rgb, image.as_mut_ptr());
        rgb.format = AVIF_RGB_FORMAT_RGBA;
        rgb.depth = 8;
        rgb.pixels = pixels as *mut u8;
        rgb.rowBytes = row_bytes_u32;
    }
    Ok(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::ManuallyDrop;

    #[cfg(test)]
    fn enable_drop_tracking() -> DropTrackingGuard {
        TRACK_DROPS.with(|t| t.set(true));
        LIVE_IMAGES.with(|c| c.set(0));
        LIVE_ENCODERS.with(|c| c.set(0));
        LIVE_RWDATA.with(|c| c.set(0));
        DropTrackingGuard
    }

    #[cfg(test)]
    struct DropTrackingGuard;

    #[cfg(test)]
    impl Drop for DropTrackingGuard {
        fn drop(&mut self) {
            TRACK_DROPS.with(|t| t.set(false));
        }
    }

    #[cfg(test)]
    fn live_images() -> usize {
        LIVE_IMAGES.with(|c| c.get())
    }

    #[cfg(test)]
    fn live_encoders() -> usize {
        LIVE_ENCODERS.with(|c| c.get())
    }

    #[cfg(test)]
    fn live_rwdata() -> usize {
        LIVE_RWDATA.with(|c| c.get())
    }

    #[test]
    fn new_rejects_zero_dimensions() {
        let err = SafeAvifImage::new(0, 10, 8, AVIF_PIXEL_FORMAT_YUV420)
            .err()
            .expect("zero width should fail");
        assert!(err.to_string().contains("Width must be greater than 0"));
    }

    #[test]
    fn new_rejects_dimension_limits() {
        let over = MAX_DIMENSION + 1;
        let err = SafeAvifImage::new(over, 10, 8, AVIF_PIXEL_FORMAT_YUV420)
            .err()
            .expect("dimensions beyond limit should fail");
        assert!(err
            .to_string()
            .contains(&format!("exceed MAX_DIMENSION {}", MAX_DIMENSION)));
    }

    #[test]
    fn create_rgb_image_rejects_pixel_overflow() {
        // MAX_DIMENSION^2 exceeds MAX_PIXELS, should fail validation.
        let mut img = SafeAvifImage::new(1, 1, 8, AVIF_PIXEL_FORMAT_YUV420).unwrap();
        let err =
            create_rgb_image(&mut img, std::ptr::null(), MAX_DIMENSION, MAX_DIMENSION).unwrap_err();
        assert!(err.to_string().contains("Pixel count"));
    }

    #[test]
    fn create_rgb_image_sets_row_bytes() {
        let mut img = SafeAvifImage::new(4, 2, 8, AVIF_PIXEL_FORMAT_YUV420).unwrap();
        let pixels: [u8; 32] = [0; 32];
        let rgb = create_rgb_image(&mut img, pixels.as_ptr(), 4, 2).unwrap();
        assert_eq!(rgb.rowBytes, 16);
        assert_eq!(rgb.format, AVIF_RGB_FORMAT_RGBA);
    }

    #[test]
    fn image_drop_happens_on_unwind() {
        let _guard = enable_drop_tracking();
        assert_eq!(live_images(), 0);

        let result = std::panic::catch_unwind(|| {
            let _img = SafeAvifImage::new(2, 2, 8, AVIF_PIXEL_FORMAT_YUV444).unwrap();
            assert_eq!(live_images(), 1);
            panic!("force unwind");
        });

        assert!(result.is_err());
        assert_eq!(live_images(), 0, "image drop should run during unwind");
    }

    #[test]
    fn image_manual_drop_runs_once() {
        let _guard = enable_drop_tracking();
        let mut img =
            ManuallyDrop::new(SafeAvifImage::new(2, 2, 8, AVIF_PIXEL_FORMAT_YUV444).unwrap());
        assert_eq!(live_images(), 1);
        let taken = unsafe { ManuallyDrop::take(&mut img) };
        drop(taken);
        assert_eq!(
            live_images(),
            0,
            "manual drop should release exactly one live image"
        );
    }

    #[test]
    fn encoder_and_rwdata_drop_once_after_move() {
        let _guard = enable_drop_tracking();
        assert_eq!(live_encoders(), 0);
        assert_eq!(live_rwdata(), 0);

        {
            let enc = SafeAvifEncoder::new().unwrap();
            let rw = SafeAvifRwData::new();
            assert_eq!(live_encoders(), 1);
            assert_eq!(live_rwdata(), 1);
            consume_encoder_and_data(enc, rw);
        }

        assert_eq!(
            live_encoders(),
            0,
            "encoder drop should execute exactly once after move"
        );
        assert_eq!(
            live_rwdata(),
            0,
            "rwdata drop should execute exactly once after move"
        );
    }

    #[test]
    fn drop_counters_stay_zero_across_tests() {
        let _guard = enable_drop_tracking();
        assert_eq!(live_images(), 0);
        assert_eq!(live_encoders(), 0);
        assert_eq!(live_rwdata(), 0);
    }

    fn consume_encoder_and_data(enc: SafeAvifEncoder, data: SafeAvifRwData) {
        drop(enc);
        drop(data);
    }

    #[test]
    fn image_drop_handles_manual_release_without_double_free() {
        let _guard = enable_drop_tracking();
        let mut img =
            SafeAvifImage::new(2, 2, 8, AVIF_PIXEL_FORMAT_YUV420).expect("image alloc should work");
        assert_eq!(live_images(), 1);

        // Simulate an external FFI consumer that takes ownership and frees the image.
        let raw = img.take_raw_for_test();
        unsafe {
            avifImageDestroy(raw);
        }
        LIVE_IMAGES.with(|c| c.set(0));

        // Drop must not double free the already-released pointer.
        drop(img);
        assert_eq!(live_images(), 0);
    }

    #[test]
    fn encoder_drop_handles_manual_release_without_double_free() {
        let _guard = enable_drop_tracking();
        let mut enc = SafeAvifEncoder::new().expect("encoder alloc should work");
        assert_eq!(live_encoders(), 1);

        let raw = enc.take_raw_for_test();
        unsafe {
            avifEncoderDestroy(raw);
        }
        LIVE_ENCODERS.with(|c| c.set(0));

        drop(enc);
        assert_eq!(live_encoders(), 0);
    }

    #[test]
    fn rwdata_drop_handles_manual_release_without_double_free() {
        let _guard = enable_drop_tracking();
        let mut data = SafeAvifRwData::new();
        assert_eq!(live_rwdata(), 1);

        // Allocate a real buffer via libavif to mirror production ownership.
        unsafe {
            let raw = data.as_mut_ptr();
            assert_eq!(avifRWDataRealloc(raw, 8), AVIF_RESULT_OK);
            avifRWDataFree(raw);
        }
        LIVE_RWDATA.with(|c| c.set(0));
        data.data.data = std::ptr::null_mut();
        data.data.size = 0;

        drop(data);
        assert_eq!(live_rwdata(), 0);
    }

    #[test]
    fn encoder_and_rwdata_drop_on_unwind() {
        let _guard = enable_drop_tracking();
        assert_eq!(live_encoders(), 0);
        assert_eq!(live_rwdata(), 0);

        let result = std::panic::catch_unwind(|| {
            let _enc = SafeAvifEncoder::new().unwrap();
            let _rw = SafeAvifRwData::new();
            assert_eq!(live_encoders(), 1);
            assert_eq!(live_rwdata(), 1);
            panic!("trigger unwind");
        });

        assert!(result.is_err());
        assert_eq!(live_encoders(), 0);
        assert_eq!(live_rwdata(), 0);
    }

    #[test]
    fn image_drop_runs_on_error_path_without_leak() {
        let _guard = enable_drop_tracking();
        assert_eq!(live_images(), 0);

        let result: Result<(), LazyImageError> = (|| {
            let _img = SafeAvifImage::new(3, 3, 8, AVIF_PIXEL_FORMAT_YUV444)?;
            Err(LazyImageError::encode_failed("avif", "synthetic failure"))
        })();

        assert!(result.is_err());
        assert_eq!(live_images(), 0);
    }
}
