use crate::engine::{detect_exif_orientation_from_reader, run_with_panic_policy};
use crate::error::LazyImageError;
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Seek, SeekFrom};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectMetadata {
    pub width: u32,
    pub height: u32,
    pub format: Option<String>,
    pub has_alpha: bool,
    pub is_animated: bool,
    pub orientation: Option<u16>,
}

fn decode_failed(context: &str, error: impl std::fmt::Display) -> LazyImageError {
    LazyImageError::decode_failed(format!("{context}: {error}"))
}

fn format_name(format: ImageFormat) -> String {
    format!("{format:?}").to_lowercase()
}

fn inspect_jpeg<R: BufRead>(reader: R) -> Result<(u32, u32, bool, bool), LazyImageError> {
    run_with_panic_policy("inspect:jpeg", || {
        // mozjpeg stops after the JPEG header. image::JpegDecoder's constructor
        // buffers the complete JPEG, which would violate inspectFile's bounded
        // header-read contract for untrusted uploads.
        let decoder = mozjpeg::Decompress::new_reader(reader)
            .map_err(|error| decode_failed("failed to read JPEG header", error))?;
        let (width, height) = decoder.size();
        let width = u32::try_from(width)
            .map_err(|error| decode_failed("JPEG width is out of range", error))?;
        let height = u32::try_from(height)
            .map_err(|error| decode_failed("JPEG height is out of range", error))?;
        Ok((width, height, false, false))
    })
}

fn inspect_png<R: BufRead + Seek>(reader: R) -> Result<(u32, u32, bool, bool), LazyImageError> {
    run_with_panic_policy("inspect:png", || {
        // PngDecoder expands palette/tRNS information in color_type(), avoiding
        // the unsafe mistake of labeling indexed transparency as opaque.
        let decoder = image::codecs::png::PngDecoder::new(reader)
            .map_err(|error| decode_failed("failed to read PNG header", error))?;
        let (width, height) = decoder.dimensions();
        let has_alpha = decoder.color_type().has_alpha();
        let is_animated = decoder
            .is_apng()
            .map_err(|error| decode_failed("failed to inspect PNG animation", error))?;
        Ok((width, height, has_alpha, is_animated))
    })
}

fn inspect_webp<R: BufRead + Seek>(reader: R) -> Result<(u32, u32, bool, bool), LazyImageError> {
    run_with_panic_policy("inspect:webp", || {
        // WebPDecoder exposes alpha and animation container flags without
        // decoding frames; both facts must be authoritative on success.
        let decoder = image::codecs::webp::WebPDecoder::new(reader)
            .map_err(|error| decode_failed("failed to read WebP header", error))?;
        let (width, height) = decoder.dimensions();
        Ok((
            width,
            height,
            decoder.color_type().has_alpha(),
            decoder.has_animation(),
        ))
    })
}

fn read_inspect_metadata<R: BufRead + Seek>(
    mut reader: R,
) -> Result<InspectMetadata, LazyImageError> {
    let format = {
        let image_reader = ImageReader::new(&mut reader)
            .with_guessed_format()
            .map_err(|error| decode_failed("failed to read image signature", error))?;
        image_reader
            .format()
            .ok_or_else(|| LazyImageError::decode_failed("failed to determine image format"))?
    };

    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| decode_failed("failed to rewind image header", error))?;

    let (width, height, has_alpha, is_animated) = match format {
        ImageFormat::Jpeg => inspect_jpeg(reader)?,
        ImageFormat::Png => inspect_png(reader)?,
        ImageFormat::WebP => inspect_webp(reader)?,
        unsupported => return Err(LazyImageError::unsupported_format(format_name(unsupported))),
    };

    Ok(InspectMetadata {
        width,
        height,
        format: Some(format_name(format)),
        has_alpha,
        is_animated,
        orientation: None,
    })
}

pub fn inspect_header_from_bytes(data: &[u8]) -> Result<InspectMetadata, LazyImageError> {
    let mut metadata = read_inspect_metadata(Cursor::new(data))?;
    let mut orientation_reader = Cursor::new(data);
    metadata.orientation = detect_exif_orientation_from_reader(&mut orientation_reader);
    Ok(metadata)
}

pub fn inspect_header_from_path(path: &str) -> Result<InspectMetadata, LazyImageError> {
    let open = || {
        File::open(path).map_err(|error| LazyImageError::file_read_failed(path.to_string(), error))
    };

    let mut metadata = read_inspect_metadata(BufReader::new(open()?))?;
    let mut orientation_reader = BufReader::new(open()?);
    metadata.orientation = detect_exif_orientation_from_reader(&mut orientation_reader);
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use std::io::{Read, Seek};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use webp::{AnimEncoder, AnimFrame, WebPConfig};

    struct CountingReader<R> {
        inner: R,
        bytes_read: Arc<AtomicUsize>,
    }

    impl<R: Read> Read for CountingReader<R> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let read = self.inner.read(output)?;
            self.bytes_read.fetch_add(read, Ordering::Relaxed);
            Ok(read)
        }
    }

    impl<R: std::io::BufRead> std::io::BufRead for CountingReader<R> {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            self.inner.fill_buf()
        }

        fn consume(&mut self, amount: usize) {
            self.bytes_read.fetch_add(amount, Ordering::Relaxed);
            self.inner.consume(amount);
        }
    }

    impl<R: Seek> Seek for CountingReader<R> {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(position)
        }
    }

    fn encode_image(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut encoded = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut encoded), format)
            .expect("test image must encode");
        encoded
    }

    fn animated_webp() -> Vec<u8> {
        let config = WebPConfig::new().expect("libwebp config must initialize");
        let first = vec![255, 0, 0, 255];
        let second = vec![0, 255, 0, 128];
        let mut encoder = AnimEncoder::new(1, 1, &config);
        encoder.add_frame(AnimFrame::from_rgba(&first, 1, 1, 0));
        encoder.add_frame(AnimFrame::from_rgba(&second, 1, 1, 100));
        encoder.encode().to_vec()
    }

    #[test]
    fn jpeg_is_opaque_and_static() {
        let jpeg = encode_image(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(3, 2, Rgb([1, 2, 3]))),
            ImageFormat::Jpeg,
        );
        let metadata = inspect_header_from_bytes(&jpeg).expect("JPEG header must inspect");

        assert_eq!((metadata.width, metadata.height), (3, 2));
        assert_eq!(metadata.format.as_deref(), Some("jpeg"));
        assert!(!metadata.has_alpha);
        assert!(!metadata.is_animated);
    }

    #[test]
    fn rgba_png_reports_alpha_without_decoding_pixels() {
        let png = encode_image(
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 3, Rgba([1, 2, 3, 4]))),
            ImageFormat::Png,
        );
        let metadata = inspect_header_from_bytes(&png).expect("RGBA PNG header must inspect");

        assert_eq!((metadata.width, metadata.height), (2, 3));
        assert!(metadata.has_alpha);
        assert!(!metadata.is_animated);
    }

    #[test]
    fn rgb_png_reports_no_alpha() {
        let png = encode_image(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 3, Rgb([1, 2, 3]))),
            ImageFormat::Png,
        );
        assert!(!inspect_header_from_bytes(&png).unwrap().has_alpha);
    }

    #[test]
    fn animated_webp_reports_animation_and_alpha() {
        let metadata = inspect_header_from_bytes(&animated_webp()).unwrap();
        assert_eq!(metadata.format.as_deref(), Some("webp"));
        assert!(metadata.has_alpha);
        assert!(metadata.is_animated);
    }

    #[test]
    fn malformed_supported_header_returns_typed_decode_failure() {
        let error = inspect_header_from_bytes(b"RIFF\x10\0\0\0WEBPVP8X").unwrap_err();
        assert!(matches!(error, LazyImageError::DecodeFailed { .. }));
    }

    #[test]
    fn orientation_is_reported_without_changing_dimensions() {
        let jpeg = include_bytes!("../test/fixtures/test_with_exif.jpg");
        let metadata = inspect_header_from_bytes(jpeg).unwrap();
        assert_eq!((metadata.width, metadata.height), (8, 8));
        assert_eq!(metadata.orientation, Some(6));
    }

    #[test]
    fn jpeg_header_inspection_does_not_read_large_scan_payload() {
        let base = encode_image(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 32, Rgb([1, 2, 3]))),
            ImageFormat::Jpeg,
        );
        let mut padded = base;
        let eoi = padded.len() - 2;
        padded.splice(eoi..eoi, std::iter::repeat_n(0u8, 4 * 1024 * 1024));

        let bytes_read = Arc::new(AtomicUsize::new(0));
        let reader = CountingReader {
            inner: BufReader::with_capacity(8 * 1024, Cursor::new(padded)),
            bytes_read: Arc::clone(&bytes_read),
        };
        let metadata = read_inspect_metadata(reader).unwrap();

        assert_eq!((metadata.width, metadata.height), (32, 32));
        assert!(
            bytes_read.load(Ordering::Relaxed) < 64 * 1024,
            "JPEG inspection must stop after bounded header reads"
        );
    }

    #[test]
    fn path_and_buffer_inspection_return_identical_metadata() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test/fixtures/test_with_exif.jpg");
        let bytes = std::fs::read(&path).unwrap();

        assert_eq!(
            inspect_header_from_path(path.to_str().unwrap()).unwrap(),
            inspect_header_from_bytes(&bytes).unwrap(),
        );
    }
}
