use image::Luma;
use qrcode::QrCode;

pub struct QrGenerator;

impl QrGenerator {
    pub fn generate_png_bytes(content: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let code = QrCode::new(content.as_bytes())?;
        let img = code
            .render::<Luma<u8>>()
            .min_dimensions(320, 320)
            .quiet_zone(true)
            .build();

        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)?;
        Ok(bytes)
    }
}
