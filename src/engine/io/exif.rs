// src/engine/io/exif.rs
//
// EXIF metadata extraction and inspection.
//
// Uses raw JPEG marker walking (allocation-free) so we can:
// - Strip GPS tags by default (privacy-first, exceeds Sharp's capabilities)
// - Detect EXIF presence without paying for a heap copy
// - Reset Orientation after auto-orient (prevents double-rotation bugs)

/// Maximum EXIF data size to process (prevent DoS from malicious inputs)
const MAX_EXIF_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Extract raw EXIF bytes from image data.
/// This is used to store EXIF for later embedding without needing to re-parse.
/// Returns the raw EXIF APP1 segment data (for JPEG) or equivalent for other formats.
///
/// Gated by `MAX_EXIF_SOURCE_BYTES` because the result is copied onto the heap.
/// For presence checks that should not be size-gated (e.g. metrics), use
/// [`has_exif`] instead.
pub fn extract_exif_raw(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || data.len() > MAX_EXIF_SOURCE_BYTES {
        return None;
    }

    // For JPEG, extract raw APP1 segment containing EXIF
    if data.starts_with(&[0xFF, 0xD8]) {
        return find_exif_segment_jpeg(data).map(<[u8]>::to_vec);
    }

    // EXIF extraction for non-JPEG formats is not yet implemented.
    // Returning None prevents the entire file buffer from being stored as "EXIF data",
    // which wastes memory and can bypass firewall metadata size limits.
    // JPEG EXIF extraction is handled above via find_exif_segment_jpeg().
    None
}

/// Lightweight EXIF-presence check.
///
/// Unlike [`extract_exif_raw`], this is **not** gated by `MAX_EXIF_SOURCE_BYTES`
/// and performs zero heap allocations — it only walks the JPEG segment list
/// looking for an `Exif\0\0` APP1 marker.
///
/// Use this in code paths (e.g. metrics finalization) that need to know whether
/// source EXIF existed without paying the cost of extracting it. Large (>8 MiB)
/// JPEGs with APP1 EXIF will be reported correctly here, whereas
/// `extract_exif_raw` would return `None` for them.
pub fn has_exif(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }

    if data.starts_with(&[0xFF, 0xD8]) {
        return find_exif_segment_jpeg(data).is_some();
    }

    // Mirror `extract_exif_raw`: non-JPEG EXIF support is not implemented yet.
    false
}

/// Detect whether a TIFF/EXIF payload (the bytes following the `Exif\0\0`
/// identifier in a JPEG APP1 segment) contains a GPS IFD pointer (tag 0x8825)
/// in IFD0.
///
/// Returns `false` on any parse error — never falsely claims GPS is present.
/// This is conservative on purpose: callers use the result to decide whether
/// "GPS was stripped" should be reported in metrics, and a false positive would
/// claim metadata was stripped when nothing changed.
pub fn exif_has_gps(exif: &[u8]) -> bool {
    // TIFF header: 2-byte byte-order + 2-byte magic (0x002A) + 4-byte IFD0 offset.
    if exif.len() < 8 {
        return false;
    }

    let is_little_endian = match &exif[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return false,
    };
    let read_u16 = |b: [u8; 2]| {
        if is_little_endian {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    };
    let read_u32 = |b: [u8; 4]| {
        if is_little_endian {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    };

    let magic = read_u16([exif[2], exif[3]]);
    if magic != 0x002A {
        return false;
    }

    let ifd0_offset = read_u32([exif[4], exif[5], exif[6], exif[7]]) as usize;
    if ifd0_offset < 8 || ifd0_offset + 2 > exif.len() {
        return false;
    }

    let num_entries = read_u16([exif[ifd0_offset], exif[ifd0_offset + 1]]) as usize;
    const ENTRY_SIZE: usize = 12;
    const GPS_IFD_POINTER_TAG: u16 = 0x8825;

    let entries_start = ifd0_offset + 2;
    let entries_bytes = match num_entries.checked_mul(ENTRY_SIZE) {
        Some(n) => n,
        None => return false,
    };
    let entries_end = match entries_start.checked_add(entries_bytes) {
        Some(end) => end,
        None => return false,
    };
    if entries_end > exif.len() {
        return false;
    }

    for n in 0..num_entries {
        let off = entries_start + n * ENTRY_SIZE;
        if read_u16([exif[off], exif[off + 1]]) == GPS_IFD_POINTER_TAG {
            return true;
        }
    }
    false
}

/// Walk JPEG markers and return a borrowed slice over the EXIF payload (the
/// bytes following the `Exif\0\0` identifier).
///
/// Allocation-free. Returns `None` when the input is not a JPEG, no EXIF APP1
/// segment is present, or the segment structure is malformed.
fn find_exif_segment_jpeg(data: &[u8]) -> Option<&[u8]> {
    const APP1: u8 = 0xE1;
    const SOS: u8 = 0xDA;
    const EOI: u8 = 0xD9;
    const EXIF_ID: &[u8] = b"Exif\0\0";

    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }

    let mut i = 2; // skip SOI

    while i + 1 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            return None;
        }
        let marker = data[i];
        i += 1;

        if marker == SOS || marker == EOI {
            break; // stop before compressed scan or explicit end
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue; // standalone marker
        }

        if i + 1 >= data.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            return None;
        }
        i += 2;
        let seg_end = i + seg_len - 2;
        if seg_end > data.len() {
            return None;
        }

        // Check for EXIF APP1 segment
        if marker == APP1 && seg_len >= 8 {
            let segment = &data[i..seg_end];
            if segment.starts_with(EXIF_ID) {
                // Return the EXIF payload borrowed from the input buffer
                // (without the `Exif\0\0` identifier).
                return Some(&segment[6..]);
            }
        }

        i = seg_end;
    }

    None
}

/// Test-only re-export so unit tests can exercise the borrowed-slice helper
/// directly without paying for the heap allocation that `extract_exif_raw`
/// performs.
#[cfg(test)]
pub(crate) fn find_exif_segment_jpeg_for_tests(data: &[u8]) -> Option<&[u8]> {
    find_exif_segment_jpeg(data)
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{
        create_minimal_jpeg, create_minimal_png, create_minimal_webp,
    };
    use super::*;

    mod exif_raw_tests {
        use super::*;

        #[test]
        fn extract_exif_raw_returns_none_for_png() {
            let png = create_minimal_png();
            assert!(
                extract_exif_raw(&png).is_none(),
                "extract_exif_raw should return None for PNG data"
            );
        }

        #[test]
        fn extract_exif_raw_returns_none_for_webp() {
            let webp = create_minimal_webp();
            assert!(
                extract_exif_raw(&webp).is_none(),
                "extract_exif_raw should return None for WebP data"
            );
        }

        #[test]
        fn extract_exif_raw_returns_none_for_jpeg_without_exif() {
            let jpeg = create_minimal_jpeg();
            assert!(
                extract_exif_raw(&jpeg).is_none(),
                "extract_exif_raw should return None for JPEG without EXIF"
            );
        }

        #[test]
        fn extract_exif_raw_returns_data_for_jpeg_with_exif() {
            let jpeg = create_minimal_jpeg();
            // Inject an EXIF APP1 segment after SOI
            let exif_header = b"Exif\0\0";
            let payload = [0xAA; 20];
            let seg_len = (2 + exif_header.len() + payload.len()) as u16;
            let mut modified = Vec::new();
            modified.extend_from_slice(&jpeg[..2]); // SOI
            modified.push(0xFF);
            modified.push(0xE1); // APP1
            modified.extend_from_slice(&seg_len.to_be_bytes());
            modified.extend_from_slice(exif_header);
            modified.extend_from_slice(&payload);
            modified.extend_from_slice(&jpeg[2..]);

            let result = extract_exif_raw(&modified);
            assert!(
                result.is_some(),
                "extract_exif_raw should return EXIF data for JPEG with EXIF"
            );
        }
    }

    mod exif_presence_tests {
        use super::*;

        /// Build a JPEG containing an EXIF APP1 segment whose payload is a
        /// valid minimal little-endian TIFF structure. `with_gps` toggles
        /// presence of a GPS IFD pointer tag in IFD0.
        fn jpeg_with_exif(with_gps: bool) -> Vec<u8> {
            let base = create_minimal_jpeg();

            // TIFF header (LE) + IFD0 with 1 or 2 entries.
            let mut tiff = Vec::new();
            tiff.extend_from_slice(b"II"); // little-endian
            tiff.extend_from_slice(&0x002A_u16.to_le_bytes()); // magic
            tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset

            // IFD0: number of entries
            let entry_count: u16 = if with_gps { 2 } else { 1 };
            tiff.extend_from_slice(&entry_count.to_le_bytes());

            // Entry 1: Orientation (0x0112), type=SHORT(3), count=1, value=1
            tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
            tiff.extend_from_slice(&3u16.to_le_bytes());
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&[1u8, 0, 0, 0]);

            if with_gps {
                // Entry 2: GPS IFD pointer (0x8825), type=LONG(4), count=1
                tiff.extend_from_slice(&0x8825_u16.to_le_bytes());
                tiff.extend_from_slice(&4u16.to_le_bytes());
                tiff.extend_from_slice(&1u32.to_le_bytes());
                // Offset points past IFD0; we don't actually parse the GPS IFD.
                tiff.extend_from_slice(&64u32.to_le_bytes());
            }

            // Next-IFD pointer (zero — terminator)
            tiff.extend_from_slice(&0u32.to_le_bytes());

            // Wrap the TIFF in a JPEG APP1 "Exif\0\0" segment.
            let exif_header = b"Exif\0\0";
            let seg_len = (2 + exif_header.len() + tiff.len()) as u16;
            let mut out = Vec::with_capacity(base.len() + tiff.len() + 16);
            out.extend_from_slice(&base[..2]); // SOI
            out.push(0xFF);
            out.push(0xE1); // APP1 marker
            out.extend_from_slice(&seg_len.to_be_bytes());
            out.extend_from_slice(exif_header);
            out.extend_from_slice(&tiff);
            out.extend_from_slice(&base[2..]);
            out
        }

        #[test]
        fn has_exif_returns_false_for_png() {
            assert!(!has_exif(&create_minimal_png()));
        }

        #[test]
        fn has_exif_returns_false_for_jpeg_without_exif() {
            assert!(!has_exif(&create_minimal_jpeg()));
        }

        #[test]
        fn has_exif_returns_true_for_jpeg_with_exif() {
            let jpeg = jpeg_with_exif(false);
            assert!(has_exif(&jpeg));
        }

        /// Regression: `extract_exif_raw` is gated by `MAX_EXIF_SOURCE_BYTES`
        /// (8 MiB), but `has_exif` must NOT be — otherwise metrics report
        /// `metadataStripped: false` for large EXIF-bearing JPEGs.
        #[test]
        fn has_exif_works_beyond_max_exif_source_bytes() {
            let mut jpeg = jpeg_with_exif(false);
            // Pad the source past the 8 MiB cap by inserting a benign APP segment.
            // Insert before SOS so the marker walker still reaches the EXIF segment.
            // We splice a large APP15 (FF EF) segment between EXIF APP1 and the rest.
            let pad_payload_len = MAX_EXIF_SOURCE_BYTES; // pushes total well over 8 MiB
            let seg_len: u16 = (pad_payload_len + 2).min(u16::MAX as usize) as u16;
            // Find the position right after the EXIF APP1 segment we injected.
            // jpeg_with_exif put SOI, then APP1, then the rest. Locate end of APP1.
            // APP1 starts at index 2: FF E1 LL LL <payload of seg_len-2>
            let app1_payload_len = u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
            // marker(2) + length(2) + payload
            let app1_end = 4 + app1_payload_len;
            // Inject FF EF <len> <pad bytes> just after APP1
            let mut padded = Vec::with_capacity(jpeg.len() + pad_payload_len + 4);
            padded.extend_from_slice(&jpeg[..app1_end]);
            // Append several large APP15 segments until we exceed the cap
            let single = {
                let mut s = Vec::with_capacity(seg_len as usize + 2);
                s.push(0xFF);
                s.push(0xEF); // APP15
                s.extend_from_slice(&seg_len.to_be_bytes());
                s.resize(s.len() + (seg_len as usize - 2), 0u8);
                s
            };
            // Repeat enough times to cross 8 MiB
            let need = MAX_EXIF_SOURCE_BYTES + single.len();
            let repeats = need / single.len() + 1;
            for _ in 0..repeats {
                padded.extend_from_slice(&single);
            }
            padded.extend_from_slice(&jpeg[app1_end..]);
            jpeg = padded;

            assert!(
                jpeg.len() > MAX_EXIF_SOURCE_BYTES,
                "test buffer must exceed MAX_EXIF_SOURCE_BYTES (got {} bytes)",
                jpeg.len()
            );

            // `extract_exif_raw` bails out due to the cap.
            assert!(
                extract_exif_raw(&jpeg).is_none(),
                "extract_exif_raw is intentionally gated by MAX_EXIF_SOURCE_BYTES"
            );
            // `has_exif` must still find the EXIF marker.
            assert!(
                has_exif(&jpeg),
                "has_exif must detect EXIF in JPEGs larger than MAX_EXIF_SOURCE_BYTES"
            );
        }

        #[test]
        fn exif_has_gps_returns_false_for_empty() {
            assert!(!exif_has_gps(&[]));
            assert!(!exif_has_gps(&[0u8; 4]));
        }

        #[test]
        fn exif_has_gps_returns_false_when_gps_absent() {
            // Pull the embedded EXIF payload back out so we test the TIFF parser
            // on real-shaped bytes.
            let jpeg = jpeg_with_exif(false);
            let payload = find_exif_segment_jpeg_for_tests(&jpeg).expect("EXIF must be present");
            assert!(!exif_has_gps(payload));
        }

        #[test]
        fn exif_has_gps_returns_true_when_gps_present() {
            let jpeg = jpeg_with_exif(true);
            let payload = find_exif_segment_jpeg_for_tests(&jpeg).expect("EXIF must be present");
            assert!(exif_has_gps(payload));
        }

        #[test]
        fn exif_has_gps_handles_big_endian() {
            // Construct a minimal big-endian TIFF with a GPS pointer.
            let mut tiff = Vec::new();
            tiff.extend_from_slice(b"MM");
            tiff.extend_from_slice(&0x002A_u16.to_be_bytes());
            tiff.extend_from_slice(&8u32.to_be_bytes()); // IFD0 offset
            tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
            tiff.extend_from_slice(&0x8825_u16.to_be_bytes()); // GPS tag
            tiff.extend_from_slice(&4u16.to_be_bytes()); // LONG
            tiff.extend_from_slice(&1u32.to_be_bytes()); // count
            tiff.extend_from_slice(&64u32.to_be_bytes()); // value/offset
            tiff.extend_from_slice(&0u32.to_be_bytes()); // next IFD
            assert!(exif_has_gps(&tiff));
        }

        #[test]
        fn exif_has_gps_rejects_unknown_byte_order() {
            let mut tiff = vec![b'X', b'X']; // not II or MM
            tiff.extend_from_slice(&[0u8; 16]);
            assert!(!exif_has_gps(&tiff));
        }
    }
}
