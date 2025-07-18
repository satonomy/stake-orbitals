use crate::*;
use image::{imageops, DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use std::io::Cursor;

// Mock PNG data - a simple 1x1 red pixel PNG
const MOCK_BASE_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0x0F, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x5C, 0xC4, 0x2A, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image_overlay() -> Result<Vec<u8>> {
        // Create a simple test base image (100x100 blue square)
        let mut base = RgbaImage::new(100, 100);
        for pixel in base.pixels_mut() {
            *pixel = image::Rgba([0, 0, 255, 255]); // Blue
        }

        // Load the actual overlay from the assets
        let overlay: RgbaImage = image::load_from_memory(OVERLAY_BYTES)?.to_rgba8();

        // Composite the overlay onto the base
        imageops::overlay(&mut base, &overlay, 0, 0);

        // Re-encode as PNG
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(base).write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;

        Ok(out)
    }

    fn create_test_image_with_mock_base() -> Result<Vec<u8>> {
        // Use the mock base PNG data and decode it
        let mut base: RgbaImage = image::load_from_memory(MOCK_BASE_PNG)?.to_rgba8();

        // Load the actual overlay from the assets
        let overlay: RgbaImage = image::load_from_memory(OVERLAY_BYTES)?.to_rgba8();

        // Resize base to match overlay dimensions if needed
        let (overlay_width, overlay_height) = overlay.dimensions();
        if base.dimensions() != overlay.dimensions() {
            base = image::imageops::resize(
                &base,
                overlay_width,
                overlay_height,
                image::imageops::FilterType::Nearest,
            );
        }

        // Composite the overlay onto the base
        imageops::overlay(&mut base, &overlay, 0, 0);

        // Re-encode as PNG
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(base).write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;

        Ok(out)
    }

    fn test_image_overlay_functionality() -> Result<Vec<u8>> {
        // Simulate the core image processing logic from get_data
        // Using mock base PNG data instead of staticcall
        let base_png_data = MOCK_BASE_PNG;

        // decode both images
        let mut base: RgbaImage = image::load_from_memory(base_png_data)?.to_rgba8();
        let overlay: RgbaImage = image::load_from_memory(OVERLAY_BYTES)?.to_rgba8();

        // If the base image is too small, resize it to accommodate the overlay
        let (overlay_width, overlay_height) = overlay.dimensions();
        let (base_width, base_height) = base.dimensions();

        if base_width < overlay_width || base_height < overlay_height {
            let new_width = std::cmp::max(base_width, overlay_width);
            let new_height = std::cmp::max(base_height, overlay_height);
            base = image::imageops::resize(
                &base,
                new_width,
                new_height,
                image::imageops::FilterType::Nearest,
            );
        }

        // composite (no resize needed)
        imageops::overlay(&mut base, &overlay, 0, 0);

        // re-encode
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(base).write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;

        Ok(out)
    }

    #[test]
    fn test_overlay_functionality() {
        let result = test_image_overlay_functionality();
        assert!(result.is_ok(), "Image overlay processing should succeed");

        let png_data = result.unwrap();
        assert!(
            !png_data.is_empty(),
            "Generated PNG data should not be empty"
        );

        // Verify it's a valid PNG by trying to decode it
        let decoded = image::load_from_memory(&png_data);
        assert!(
            decoded.is_ok(),
            "Generated PNG should be valid and decodable"
        );

        println!("Successfully generated PNG with {} bytes", png_data.len());
    }

    #[test]
    fn test_blue_base_with_overlay() {
        let result = create_test_image_overlay();
        assert!(
            result.is_ok(),
            "Blue base image with overlay should succeed"
        );

        let png_data = result.unwrap();
        assert!(
            !png_data.is_empty(),
            "Generated PNG data should not be empty"
        );

        // Verify it's a valid PNG
        let decoded = image::load_from_memory(&png_data);
        assert!(
            decoded.is_ok(),
            "Generated PNG should be valid and decodable"
        );

        println!(
            "Successfully generated blue base PNG with overlay: {} bytes",
            png_data.len()
        );
    }

    #[test]
    fn test_mock_base_with_overlay() {
        let result = create_test_image_with_mock_base();
        assert!(
            result.is_ok(),
            "Mock base image with overlay should succeed"
        );

        let png_data = result.unwrap();
        assert!(
            !png_data.is_empty(),
            "Generated PNG data should not be empty"
        );

        // Verify it's a valid PNG
        let decoded = image::load_from_memory(&png_data);
        assert!(
            decoded.is_ok(),
            "Generated PNG should be valid and decodable"
        );

        println!(
            "Successfully generated mock base PNG with overlay: {} bytes",
            png_data.len()
        );
    }

    #[test]
    fn test_overlay_bytes_valid() {
        // Test that our overlay asset is valid
        let decoded = image::load_from_memory(OVERLAY_BYTES);
        assert!(decoded.is_ok(), "Overlay PNG should be valid");

        let img = decoded.unwrap();
        let (width, height) = img.dimensions();
        println!("Overlay dimensions: {}x{}", width, height);
        assert!(
            width > 0 && height > 0,
            "Overlay should have positive dimensions"
        );
    }
}
