// src/codecs/avif_safe.rs
//
// Safe abstractions for libavif FFI operations.
// This module provides RAII-based wrappers that hide raw pointers and
// eliminate unsafe blocks from the calling code.

use crate::error::LazyImageError;
use libavif_sys::*;

/// Safe wrapper for avifImage that manages its lifetime using RAII.
/// This eliminates the need for unsafe blocks when working with AVIF images.
pub struct SafeAvifImage {
    ptr: *mut avifImage,
}

impl SafeAvifImage {
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
        let ptr = unsafe { avifImageCreate(width, height, depth, pixel_format) };
        if ptr.is_null() {
            return Err(LazyImageError::encode_failed(
                "avif",
                "Failed to create AVIF image",
            ));
        }
        Ok(Self { ptr })
    }

    /// Set color properties for the image.
    pub fn set_color_properties(
        &mut self,
        primaries: u16,
        transfer: u16,
        matrix: u16,
        yuv_range: avifRange,
    ) {
        unsafe {
            (*self.ptr).colorPrimaries = primaries;
            (*self.ptr).transferCharacteristics = transfer;
            (*self.ptr).matrixCoefficients = matrix;
            (*self.ptr).yuvRange = yuv_range;
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
        let result = unsafe { avifImageSetProfileICC(self.ptr, icc.as_ptr(), icc.len()) };
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
        let result = unsafe { avifImageAllocatePlanes(self.ptr, planes) };
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
        let result = unsafe { avifImageRGBToYUV(self.ptr, rgb) };
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
    /// The caller must ensure that the alpha plane has been allocated
    /// and that the pointer is valid for the lifetime of the image.
    pub unsafe fn alpha_plane_mut(&mut self) -> *mut u8 {
        (*self.ptr).alphaPlane
    }

    /// Get the alpha row bytes.
    pub fn alpha_row_bytes(&self) -> usize {
        unsafe { (*self.ptr).alphaRowBytes as usize }
    }

    /// Get the raw pointer to the avifImage.
    /// This is only exposed when absolutely necessary for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is not used after the
    /// SafeAvifImage is dropped, and that it is not used concurrently.
    pub unsafe fn as_ptr(&self) -> *const avifImage {
        self.ptr
    }

    /// Get a mutable raw pointer to the avifImage.
    /// This is only exposed when absolutely necessary for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is not used after the
    /// SafeAvifImage is dropped, and that it is not used concurrently.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut avifImage {
        self.ptr
    }
}

impl Drop for SafeAvifImage {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                avifImageDestroy(self.ptr);
            }
        }
    }
}

/// Safe wrapper for avifEncoder that manages its lifetime using RAII.
pub struct SafeAvifEncoder {
    ptr: *mut avifEncoder,
}

impl SafeAvifEncoder {
    /// Create a new AVIF encoder.
    ///
    /// # Returns
    /// Returns `Ok(SafeAvifEncoder)` on success, or an error if encoder creation fails.
    pub fn new() -> Result<Self, LazyImageError> {
        let ptr = unsafe { avifEncoderCreate() };
        if ptr.is_null() {
            return Err(LazyImageError::encode_failed(
                "avif",
                "Failed to create AVIF encoder",
            ));
        }
        Ok(Self { ptr })
    }

    /// Set encoder quality settings.
    ///
    /// # Arguments
    /// * `quality` - Quality value (0-100)
    /// * `quality_alpha` - Alpha quality value (0-100)
    /// * `speed` - Encoding speed (0-10, where 0 is slowest/best)
    /// * `max_threads` - Maximum number of threads to use
    pub fn configure(&mut self, quality: u8, quality_alpha: u8, speed: i32, max_threads: i32) {
        unsafe {
            (*self.ptr).quality = quality as i32;
            (*self.ptr).qualityAlpha = quality_alpha as i32;
            (*self.ptr).speed = speed;
            (*self.ptr).maxThreads = max_threads;
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
        let result =
            unsafe { avifEncoderAddImage(self.ptr, image.as_mut_ptr(), duration, add_image_flags) };
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
        let result = unsafe { avifEncoderFinish(self.ptr, output.as_mut_ptr()) };
        if result != AVIF_RESULT_OK {
            return Err(LazyImageError::encode_failed(
                "avif",
                format!("Failed to finish encoding: {:?}", result),
            ));
        }
        Ok(())
    }
}

impl Drop for SafeAvifEncoder {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                avifEncoderDestroy(self.ptr);
            }
        }
    }
}

/// Safe wrapper for avifRWData that manages its lifetime using RAII.
pub struct SafeAvifRwData {
    data: avifRWData,
}

impl SafeAvifRwData {
    /// Create a new empty avifRWData structure.
    pub fn new() -> Self {
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

    /// Copy the encoded data into a Vec<u8>.
    ///
    /// # Returns
    /// Returns a Vec<u8> containing a copy of the encoded AVIF data.
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
    _height: u32,
) -> avifRGBImage {
    let mut rgb: avifRGBImage = unsafe { std::mem::zeroed() };
    unsafe {
        avifRGBImageSetDefaults(&mut rgb, image.as_mut_ptr());
        rgb.format = AVIF_RGB_FORMAT_RGBA;
        rgb.depth = 8;
        rgb.pixels = pixels as *mut u8;
        rgb.rowBytes = (width * 4) as u32;
    }
    rgb
}
