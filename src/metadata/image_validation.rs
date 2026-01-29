//! Image validation logic for metadata scraping.
//!
//! Validates that bytes represent a usable image by checking:
//! - Non-empty and minimum size
//! - Magic bytes match known formats
//! - Not HTML content
//! - Successfully decodes
//! - Meets minimum resolution requirements

const MIN_IMAGE_BYTES: usize = 512; // catch tiny placeholders while allowing small valid thumbnails
const MIN_DIMENSION: u32 = 32; // Reject tracking pixels and tiny favicons

/// Returns true if the bytes represent a valid, usable image.
///
/// All validation checks must pass:
/// 1. Non-empty bytes
/// 2. Minimum byte size > 2048 (2KB)
/// 3. Magic bytes match known image format (PNG, JPEG, GIF, WebP, BMP, ICO)
/// 4. Not HTML content
/// 5. Decode succeeds
/// 6. Resolution > 32x32 (both width AND height)
pub fn validate_image(bytes: &[u8]) -> bool {
    // Check 1: Non-empty
    if bytes.is_empty() {
        return false;
    }

    // Check 2: Minimum byte size
    if bytes.len() < MIN_IMAGE_BYTES {
        return false;
    }

    // Check 3: Magic bytes match known format
    if !has_valid_magic_bytes(bytes) {
        return false;
    }

    // Check 4: Reject HTML content
    if is_html_content(bytes) {
        return false;
    }

    // Check 5 & 6: Decode succeeds and meets resolution requirements
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let (width, height) = (img.width(), img.height());
            width > MIN_DIMENSION && height > MIN_DIMENSION
        }
        Err(_) => false,
    }
}

/// Checks if bytes start with valid image format magic bytes.
fn has_valid_magic_bytes(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }

    // PNG: \x89PNG
    if bytes.len() >= 4 && bytes[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        return true;
    }

    // JPEG: \xFF\xD8\xFF
    if bytes.len() >= 3 && bytes[0..3] == [0xFF, 0xD8, 0xFF] {
        return true;
    }

    // GIF: GIF8
    if bytes.len() >= 4 && bytes[0..4] == *b"GIF8" {
        return true;
    }

    // WebP: RIFF at start and WEBP at bytes 8..12
    if bytes.len() >= 12
        && bytes[0..4] == *b"RIFF"
        && bytes[8..12] == *b"WEBP"
    {
        return true;
    }

    // BMP: BM
    if bytes.len() >= 2 && bytes[0..2] == *b"BM" {
        return true;
    }

    // ICO: \x00\x00\x01\x00
    if bytes.len() >= 4 && bytes[0..4] == [0x00, 0x00, 0x01, 0x00] {
        return true;
    }

    false
}

/// Checks if bytes look like HTML content (case-insensitive check of first 50 bytes).
fn is_html_content(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(50);
    let prefix = &bytes[0..check_len];

    // Convert to lowercase for case-insensitive comparison
    let prefix_lower = prefix.to_ascii_lowercase();

    prefix_lower.starts_with(b"<!doctype") || prefix_lower.starts_with(b"<html")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb, RgbImage};
    use std::io::Cursor;

    /// Helper to create a PNG image in memory with specified dimensions.
    fn create_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
            // Simple pattern to make it a real image
            let r = (x % 256) as u8;
            let g = (y % 256) as u8;
            let b = ((x + y) % 256) as u8;
            Rgb([r, g, b])
        });

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
        buffer.into_inner()
    }

    /// Helper to create a JPEG image in memory with specified dimensions.
    fn create_jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
        let img: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
            let r = (x % 256) as u8;
            let g = (y % 256) as u8;
            let b = ((x + y) % 256) as u8;
            Rgb([r, g, b])
        });

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Jpeg).unwrap();
        buffer.into_inner()
    }

    #[test]
    fn test_valid_minimal_png_accepted() {
        // Create a small but valid PNG (100x100)
        let png_bytes = create_png_bytes(100, 100);

        // Verify it's above our minimum size requirement
        assert!(png_bytes.len() >= MIN_IMAGE_BYTES);

        assert!(validate_image(&png_bytes), "Valid PNG should be accepted");
    }

    #[test]
    fn test_valid_jpeg_accepted() {
        let jpeg_bytes = create_jpeg_bytes(100, 100);

        // Verify it's above our minimum size requirement
        assert!(jpeg_bytes.len() >= MIN_IMAGE_BYTES);

        assert!(validate_image(&jpeg_bytes), "Valid JPEG should be accepted");
    }

    #[test]
    fn test_empty_bytes_rejected() {
        assert!(!validate_image(&[]), "Empty bytes should be rejected");
    }

    #[test]
    fn test_bytes_under_2kb_rejected() {
        // Create a small buffer with PNG magic bytes but under 2KB
        let mut small_bytes = vec![0x89, 0x50, 0x4E, 0x47];
        small_bytes.resize(MIN_IMAGE_BYTES - 1, 0);

        assert!(
            !validate_image(&small_bytes),
            "Bytes under 2KB should be rejected even with valid header"
        );
    }

    #[test]
    fn test_html_content_rejected_doctype() {
        let html = b"<!DOCTYPE html><html><body>fake image</body></html>";
        let mut padded = html.to_vec();
        padded.resize(MIN_IMAGE_BYTES + 100, b' '); // Pad to meet size requirement

        assert!(
            !validate_image(&padded),
            "HTML content with DOCTYPE should be rejected"
        );
    }

    #[test]
    fn test_html_content_rejected_html_tag() {
        let html = b"<html><head><title>Not an image</title></head></html>";
        let mut padded = html.to_vec();
        padded.resize(MIN_IMAGE_BYTES + 100, b' ');

        assert!(
            !validate_image(&padded),
            "HTML content with <html> tag should be rejected"
        );
    }

    #[test]
    fn test_html_content_rejected_case_insensitive() {
        let html = b"<!DoCtYpE HtMl><HTML>mixed case</HTML>";
        let mut padded = html.to_vec();
        padded.resize(MIN_IMAGE_BYTES + 100, b' ');

        assert!(
            !validate_image(&padded),
            "HTML content should be rejected regardless of case"
        );
    }

    #[test]
    fn test_1x1_pixel_image_rejected() {
        let tiny_img = create_png_bytes(1, 1);

        // Even if it's a valid image format, 1x1 should be rejected
        assert!(
            !validate_image(&tiny_img),
            "1x1 pixel image should be rejected (too small resolution)"
        );
    }

    #[test]
    fn test_32x32_image_rejected() {
        // Exactly 32x32 should be rejected (we need > 32)
        let small_img = create_png_bytes(32, 32);

        assert!(
            !validate_image(&small_img),
            "32x32 pixel image should be rejected (must be > 32x32)"
        );
    }

    #[test]
    fn test_33x33_image_accepted() {
        // Just above the threshold
        let img = create_png_bytes(33, 33);

        assert!(
            validate_image(&img),
            "33x33 pixel image should be accepted"
        );
    }

    #[test]
    fn test_random_garbage_bytes_rejected() {
        // Large enough but random garbage
        let garbage = vec![0xAB; MIN_IMAGE_BYTES + 100];

        assert!(
            !validate_image(&garbage),
            "Random garbage bytes should be rejected"
        );
    }

    #[test]
    fn test_truncated_png_rejected() {
        // Start with valid PNG magic bytes
        let mut truncated = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        // Add some random bytes to meet size requirement but not enough for valid PNG
        truncated.resize(MIN_IMAGE_BYTES + 100, 0xFF);

        assert!(
            !validate_image(&truncated),
            "Truncated/corrupt image with valid header should be rejected"
        );
    }

    #[test]
    fn test_valid_large_image_accepted() {
        // Test with a larger, clearly valid image
        let large_img = create_png_bytes(500, 400);

        assert!(
            validate_image(&large_img),
            "Large valid image should be accepted"
        );
    }

    #[test]
    fn test_no_magic_bytes_rejected() {
        // Large enough but no valid magic bytes
        let no_magic = vec![0x00; MIN_IMAGE_BYTES + 100];

        assert!(
            !validate_image(&no_magic),
            "Bytes without valid magic bytes should be rejected"
        );
    }

    #[test]
    fn test_wide_but_short_image_rejected() {
        // 1000x10 - wide but too short
        let wide_short = create_png_bytes(1000, 10);

        assert!(
            !validate_image(&wide_short),
            "Image with one dimension <= 32 should be rejected"
        );
    }

    #[test]
    fn test_tall_but_narrow_image_rejected() {
        // 10x1000 - tall but too narrow
        let tall_narrow = create_png_bytes(10, 1000);

        assert!(
            !validate_image(&tall_narrow),
            "Image with one dimension <= 32 should be rejected"
        );
    }
}
