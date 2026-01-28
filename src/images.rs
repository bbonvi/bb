//! Image compression utilities for WebP conversion
//!
//! Handles resizing images to max dimensions and encoding to lossy WebP format.

use anyhow::{Context, Result};
use image::{GenericImageView, ImageFormat};

/// Result of image compression operation
#[derive(Debug)]
pub struct CompressionResult {
    /// Compressed image data (WebP format)
    pub data: Vec<u8>,
    /// Original image dimensions (width, height)
    pub original_dimensions: (u32, u32),
    /// New dimensions after resize (width, height)
    pub new_dimensions: (u32, u32),
    /// Whether the image was resized
    pub was_resized: bool,
}

/// Check if data starts with WebP magic bytes (RIFF....WEBP)
pub fn is_webp(data: &[u8]) -> bool {
    data.len() >= 12
        && data[0..4] == *b"RIFF"
        && data[8..12] == *b"WEBP"
}

/// Determine if image should be processed based on current dimensions and format
///
/// Returns true if:
/// - Image is not already WebP, OR
/// - Image dimensions exceed max_dimension
pub fn should_process(data: &[u8], max_dimension: u32) -> bool {
    // Non-WebP images always need processing
    if !is_webp(data) {
        return true;
    }

    // For WebP, check if dimensions exceed max
    match image::load_from_memory(data) {
        Ok(img) => {
            let (w, h) = img.dimensions();
            w > max_dimension || h > max_dimension
        }
        Err(_) => true, // Can't parse - try to process anyway
    }
}

/// Compress an image: resize if needed, convert to WebP lossy format
///
/// # Arguments
/// * `data` - Raw image bytes (any format supported by `image` crate)
/// * `max_dimension` - Maximum width or height; larger images are scaled down
/// * `quality` - WebP quality (1-100, 85 recommended for good balance)
///
/// # Returns
/// CompressionResult with compressed data and dimension info
pub fn compress_image(data: &[u8], max_dimension: u32, quality: u8) -> Result<CompressionResult> {
    let img = image::load_from_memory(data)
        .context("Failed to decode image")?;

    let (orig_w, orig_h) = img.dimensions();
    let original_dimensions = (orig_w, orig_h);

    // Calculate new dimensions maintaining aspect ratio
    let (new_w, new_h, was_resized) = if orig_w > max_dimension || orig_h > max_dimension {
        let scale = (max_dimension as f64) / (orig_w.max(orig_h) as f64);
        let new_w = ((orig_w as f64) * scale).round() as u32;
        let new_h = ((orig_h as f64) * scale).round() as u32;
        (new_w.max(1), new_h.max(1), true)
    } else {
        (orig_w, orig_h, false)
    };

    // Resize if needed using Lanczos3 filter for quality
    let processed = if was_resized {
        img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Convert to RGBA8 for WebP encoder
    let rgba = processed.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Encode to WebP
    let encoder = webp::Encoder::from_rgba(&rgba, width, height);
    let webp_data = encoder.encode(quality as f32);

    Ok(CompressionResult {
        data: webp_data.to_vec(),
        original_dimensions,
        new_dimensions: (new_w, new_h),
        was_resized,
    })
}

/// Get image dimensions without fully decoding
pub fn get_dimensions(data: &[u8]) -> Result<(u32, u32)> {
    let img = image::load_from_memory(data)
        .context("Failed to decode image for dimensions")?;
    Ok(img.dimensions())
}

/// Detect image format from bytes
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    image::guess_format(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid PNG: 1x1 red pixel
    fn create_test_png() -> Vec<u8> {
        let mut img = image::RgbaImage::new(1, 1);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));

        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, ImageFormat::Png).unwrap();
        buf
    }

    // Create larger test PNG
    fn create_large_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
        });

        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, ImageFormat::Png).unwrap();
        buf
    }

    #[test]
    fn test_is_webp_valid() {
        // Valid WebP header: RIFF....WEBP
        let webp_header = b"RIFF\x00\x00\x00\x00WEBP";
        assert!(is_webp(webp_header));
    }

    #[test]
    fn test_is_webp_invalid() {
        let png_data = create_test_png();
        assert!(!is_webp(&png_data));
    }

    #[test]
    fn test_is_webp_too_short() {
        assert!(!is_webp(&[1, 2, 3]));
        assert!(!is_webp(&[]));
    }

    #[test]
    fn test_compress_small_image_no_resize() {
        let png = create_test_png();
        let result = compress_image(&png, 600, 85).unwrap();

        assert!(!result.was_resized);
        assert_eq!(result.original_dimensions, (1, 1));
        assert_eq!(result.new_dimensions, (1, 1));
        assert!(is_webp(&result.data));
    }

    #[test]
    fn test_compress_large_image_resizes() {
        let png = create_large_png(1200, 800);
        let result = compress_image(&png, 600, 85).unwrap();

        assert!(result.was_resized);
        assert_eq!(result.original_dimensions, (1200, 800));
        // Should scale to 600x400 maintaining aspect ratio
        assert!(result.new_dimensions.0 <= 600);
        assert!(result.new_dimensions.1 <= 600);
        assert!(is_webp(&result.data));
    }

    #[test]
    fn test_compress_portrait_image() {
        let png = create_large_png(400, 1200);
        let result = compress_image(&png, 600, 85).unwrap();

        assert!(result.was_resized);
        // Height was limiting factor, should scale to ~200x600
        assert!(result.new_dimensions.0 <= 600);
        assert!(result.new_dimensions.1 <= 600);
    }

    #[test]
    fn test_compress_exact_max_dimension() {
        let png = create_large_png(600, 400);
        let result = compress_image(&png, 600, 85).unwrap();

        // 600 is exactly max, no resize needed
        assert!(!result.was_resized);
        assert_eq!(result.new_dimensions, (600, 400));
    }

    #[test]
    fn test_compress_invalid_data_fails() {
        let garbage = vec![1, 2, 3, 4, 5];
        let result = compress_image(&garbage, 600, 85);
        assert!(result.is_err());
    }

    #[test]
    fn test_should_process_non_webp() {
        let png = create_test_png();
        assert!(should_process(&png, 600));
    }

    #[test]
    fn test_get_dimensions() {
        let png = create_large_png(320, 240);
        let dims = get_dimensions(&png).unwrap();
        assert_eq!(dims, (320, 240));
    }

    #[test]
    fn test_detect_format_png() {
        let png = create_test_png();
        assert_eq!(detect_format(&png), Some(ImageFormat::Png));
    }

    #[test]
    fn test_quality_affects_size() {
        let png = create_large_png(400, 400);
        let high_q = compress_image(&png, 600, 95).unwrap();
        let low_q = compress_image(&png, 600, 50).unwrap();

        // Lower quality should produce smaller output
        assert!(low_q.data.len() < high_q.data.len());
    }
}
