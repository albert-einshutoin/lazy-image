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
    static TRACK_DROPS: Cell<bool> = const { Cell::new(false) };
    static LIVE_IMAGES: Cell<usize> = const { Cell::new(0) };
    static LIVE_ENCODERS: Cell<usize> = const { Cell::new(0) };
    static LIVE_RWDATA: Cell<usize> = const { Cell::new(0) };
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
        // SAFETY: `avifImageCreate` accepts any non-zero width, height, depth, and
        // pixel-format values — all validated above. It returns either a
        // heap-allocated `avifImage` or NULL; we check for NULL via `NonNull::new`
        // immediately after, and the resulting pointer is uniquely owned by the
        // returned `SafeAvifImage` which frees it in `Drop`.
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
    ///
    /// Returns an error if the image pointer was already released, matching the
    /// fallible contract of the peer methods (`set_icc_profile`,
    /// `allocate_planes`) instead of panicking via `expect`.
    pub fn set_color_properties(
        &mut self,
        primaries: u16,
        transfer: u16,
        matrix: u16,
        yuv_range: avifRange,
    ) -> Result<(), LazyImageError> {
        let image = self.ptr.ok_or_else(|| {
            LazyImageError::encode_failed("avif", "AVIF image pointer was released")
        })?;
        // SAFETY: `image` is a non-null, libavif-owned `avifImage` that this
        // `SafeAvifImage` exclusively owns (`&mut self`), so writing its color
        // metadata fields is sound and free of aliasing.
        unsafe {
            let raw = image.as_ptr();
            (*raw).colorPrimaries = primaries;
            (*raw).transferCharacteristics = transfer;
            (*raw).matrixCoefficients = matrix;
            (*raw).yuvRange = yuv_range;
        }
        Ok(())
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
        // SAFETY: `image` is a non-null, libavif-owned `avifImage` uniquely owned by
        // this `SafeAvifImage` (`&mut self`). `icc.as_ptr()` and `icc.len()` describe
        // a valid, initialized byte slice that outlives this call; libavif copies the
        // data internally, so no aliasing or lifetime issue arises.
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
        // SAFETY: `image` is a non-null, libavif-owned `avifImage` uniquely owned by
        // this `SafeAvifImage` (`&mut self`). `planes` is a plain integer flag; the
        // function only writes into libavif-managed heap memory it already owns,
        // so this call is free of aliasing or dangling-pointer hazards.
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
        // SAFETY: `image` is a non-null, libavif-owned `avifImage` uniquely owned by
        // this `SafeAvifImage` (`&mut self`). `rgb` is a pointer to a fully initialised
        // `avifRGBImage` created via `create_rgb_image`, whose pixel buffer remains
        // valid for the duration of this call per that function's documented contract.
        // libavif reads through `rgb` and writes into the YUV planes it already owns,
        // so no aliasing or use-after-free can occur.
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
        // SAFETY: `image` is a non-null, libavif-owned `avifImage` uniquely owned by
        // this `SafeAvifImage` (`&mut self`). Dereferencing the pointer to read the
        // `alphaPlane` field is sound because the object is fully initialised by
        // libavif and `&mut self` ensures exclusive access.
        let plane_ptr = unsafe { (*image.as_ptr()).alphaPlane };
        NonNull::new(plane_ptr)
            .ok_or_else(|| LazyImageError::encode_failed("avif", "Alpha plane is not allocated"))
    }

    /// Get the alpha row bytes.
    pub fn alpha_row_bytes(&self) -> usize {
        let image = self
            .ptr
            .expect("SafeAvifImage pointer was released before querying alpha rows");
        // SAFETY: `image` is a non-null, libavif-owned `avifImage` uniquely owned by
        // this `SafeAvifImage`. `&self` ensures no concurrent mutation exists.
        // Dereferencing to read the plain `alphaRowBytes` field of a fully
        // initialised struct is sound.
        unsafe { (*image.as_ptr()).alphaRowBytes as usize }
    }

    /// Get the raw pointer to the avifImage.
    /// This is only exposed when absolutely necessary for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure that the pointer is not used after the
    /// SafeAvifImage is dropped, and that it is not used concurrently.
    pub unsafe fn as_ptr(&self) -> *const avifImage {
        // SAFETY: The caller guarantees (per the `# Safety` doc) that the returned
        // pointer is not used after this `SafeAvifImage` is dropped and that no
        // concurrent mutable access exists. `self.ptr` is `Some` as long as this
        // wrapper is live; `expect` enforces this invariant at runtime.
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
        // SAFETY: The caller guarantees (per the `# Safety` doc) that the returned
        // pointer is not used after this `SafeAvifImage` is dropped and that no
        // concurrent access exists. `&mut self` ensures exclusive access at the Rust
        // level. `self.ptr` is `Some` as long as this wrapper is live; `expect`
        // enforces this invariant at runtime.
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
            // SAFETY: `ptr` is non-null (it came from `NonNull`) and is the unique
            // owner of the `avifImage` allocation created by libavif. `self.ptr.take()`
            // sets the field to `None`, guaranteeing this destructor path runs at most
            // once, so `avifImageDestroy` is called exactly once with no double-free.
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
        // SAFETY: `avifEncoderCreate` takes no arguments and returns either a
        // heap-allocated `avifEncoder` or NULL; we check for NULL via `NonNull::new`
        // immediately after. The resulting pointer is uniquely owned by the returned
        // `SafeAvifEncoder` which frees it in `Drop` via `avifEncoderDestroy`.
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
    /// * `quality` - Quality value (1-100)
    /// * `quality_alpha` - Alpha quality value (1-100)
    /// * `speed` - Encoding speed (0-10, where 0 is slowest/best)
    /// * `max_threads` - Maximum number of threads to use
    pub fn configure(
        &mut self,
        quality: u8,
        quality_alpha: u8,
        speed: i32,
        max_threads: i32,
    ) -> Result<(), LazyImageError> {
        let encoder = self
            .ptr
            .ok_or_else(|| LazyImageError::encode_failed("avif", "AVIF encoder was released"))?;
        // SAFETY: `encoder` is a non-null, libavif-owned `avifEncoder` uniquely
        // owned by this `SafeAvifEncoder` (`&mut self`); writing its plain
        // configuration fields is sound with no aliasing.
        unsafe {
            let raw = encoder.as_ptr();
            (*raw).quality = quality as i32;
            (*raw).qualityAlpha = quality_alpha as i32;
            (*raw).speed = speed;
            (*raw).maxThreads = max_threads;
        }
        Ok(())
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
        // SAFETY: `encoder` is a non-null, libavif-owned `avifEncoder` uniquely
        // owned by this `SafeAvifEncoder` (`&mut self`). `image.as_mut_ptr()` yields
        // the non-null, live `avifImage` pointer uniquely borrowed through `&mut
        // SafeAvifImage`; no other reference to it exists during this call. `duration`
        // and `add_image_flags` are plain integers passed by value.
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
        // SAFETY: `encoder` is a non-null, libavif-owned `avifEncoder` uniquely owned
        // by this `SafeAvifEncoder` (`&mut self`). `output.as_mut_ptr()` returns a
        // valid pointer to the `avifRWData` embedded in `SafeAvifRwData`, uniquely
        // borrowed through `&mut SafeAvifRwData`; libavif writes the encoded bytes
        // into that buffer and takes ownership of the allocation it creates there,
        // which `Drop for SafeAvifRwData` will free via `avifRWDataFree`.
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
            // SAFETY: `ptr` is non-null (it came from `NonNull`) and is the unique
            // owner of the `avifEncoder` allocation created by libavif.
            // `self.ptr.take()` sets the field to `None`, guaranteeing this destructor
            // path runs at most once, so `avifEncoderDestroy` is called exactly once
            // with no double-free.
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
            // SAFETY: `avifRWData` is a plain C struct (a data pointer and a size)
            // with no invariants beyond those maintained by libavif itself. An
            // all-zero bit pattern is the correct empty / NULL state recognized by
            // libavif's `avifRWDataFree` and `avifEncoderFinish`.
            data: unsafe { std::mem::zeroed() },
        }
    }

    /// Get the encoded data as a byte slice.
    ///
    /// # Returns
    /// Returns a byte slice containing the encoded AVIF data.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: When `self.data.data` is non-null and `self.data.size > 0`, the
        // pointer was written by libavif (`avifEncoderFinish`) and refers to a
        // contiguous, initialized byte array of exactly `self.data.size` bytes.
        // `&self` ensures no concurrent mutation exists. The returned slice borrows
        // from `self`, so it cannot outlive the `SafeAvifRwData` that owns the buffer.
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
        // SAFETY: The caller guarantees (per the `# Safety` doc) that the returned
        // pointer is not used after this `SafeAvifRwData` is dropped. `&mut self`
        // provides exclusive access to `self.data`, so the returned raw pointer is
        // not aliased by any other live reference for the duration of the caller's
        // use. `self.data` is a stack-embedded struct, so the pointer is always valid
        // and properly aligned.
        &mut self.data
    }
}

impl Drop for SafeAvifRwData {
    fn drop(&mut self) {
        // SAFETY: `self.data` is either zero-initialised (empty) or was populated by
        // libavif via `avifEncoderFinish`. `avifRWDataFree` correctly handles the
        // zero/null case as a no-op. `Drop` runs at most once (Rust guarantees
        // single-drop semantics), so the buffer is freed exactly once with no
        // double-free. `&mut self.data` is exclusively borrowed here; no other
        // reference to it exists.
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
///
/// # Safety
///
/// The returned [`avifRGBImage`] borrows `pixels` as its backing store; the
/// caller must guarantee that:
/// - `pixels` points to at least `width * 4 * height` initialized bytes laid
///   out as tightly-packed RGBA8 rows, and
/// - that buffer remains valid and is **not** mutated for as long as the
///   returned `avifRGBImage` is used (e.g. passed to `rgb_to_yuv`).
///
/// The function itself only reads through the pointer indirectly via libavif;
/// it never writes through it.
pub unsafe fn create_rgb_image(
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

    // SAFETY: `avifRGBImage` is a plain C struct that is valid when
    // zero-initialized; `avifRGBImageSetDefaults` then fills it from the owned
    // `image`. `image.as_mut_ptr()` is a live, non-null libavif image.
    let mut rgb: avifRGBImage = unsafe { std::mem::zeroed() };
    // SAFETY: see this function's `# Safety` contract. `pixels` is non-null
    // (checked above) and points to `row_bytes_u32 * height` valid RGBA8 bytes
    // owned by the caller for the lifetime of `rgb`. We store it as `*mut u8`
    // only because the C field is typed that way; libavif's RGB→YUV conversion
    // treats this buffer as read-only and never writes through it, so deriving a
    // `*mut` from the caller's (possibly shared) buffer is never used to mutate.
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
        // SAFETY: validation rejects the oversized dimensions before the null
        // pointer is ever dereferenced, so no invalid memory is accessed.
        let err =
            unsafe { create_rgb_image(&mut img, std::ptr::null(), MAX_DIMENSION, MAX_DIMENSION) }
                .unwrap_err();
        assert!(err.to_string().contains("Pixel count"));
    }

    #[test]
    fn create_rgb_image_sets_row_bytes() {
        let mut img = SafeAvifImage::new(4, 2, 8, AVIF_PIXEL_FORMAT_YUV420).unwrap();
        let pixels: [u8; 32] = [0; 32];
        // SAFETY: `pixels` holds exactly 4 * 2 * 4 = 32 RGBA8 bytes and outlives
        // `rgb`, satisfying `create_rgb_image`'s contract.
        let rgb = unsafe { create_rgb_image(&mut img, pixels.as_ptr(), 4, 2) }.unwrap();
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
        // SAFETY: `img` was just created above and has not been moved or dropped.
        // `ManuallyDrop::take` moves the inner value out of the `ManuallyDrop`
        // wrapper; after this call `img` must not be used again. We immediately
        // `drop(taken)` to run the `SafeAvifImage` destructor exactly once.
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
        // SAFETY: `raw` is the non-null `avifImage` pointer that was just extracted
        // from `img` via `take_raw_for_test`, which sets `img.ptr` to `None`. We are
        // the sole owner of this pointer at this point; calling `avifImageDestroy`
        // exactly once here frees it without double-free when `img` is dropped below.
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
        // SAFETY: `raw` is the non-null `avifEncoder` pointer that was just extracted
        // from `enc` via `take_raw_for_test`, which sets `enc.ptr` to `None`. We are
        // the sole owner of this pointer at this point; calling `avifEncoderDestroy`
        // exactly once here frees it without double-free when `enc` is dropped below.
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
        // SAFETY: `data.as_mut_ptr()` returns a valid, exclusively-borrowed pointer to
        // the `avifRWData` embedded in `data`. `avifRWDataRealloc` allocates an 8-byte
        // buffer managed by libavif. We then immediately call `avifRWDataFree` on that
        // same pointer while it is still the sole owner, freeing the buffer once. After
        // this block we zero the fields so the `Drop` impl's call to `avifRWDataFree`
        // is a safe no-op on a null pointer.
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
