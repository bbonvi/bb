use std::io::Cursor;

use image::{
    imageops::{resize, FilterType},
    ImageFormat, ImageReader,
};

pub fn convert(
    file: Vec<u8>,
    dims: Option<(u32, u32)>,
    preserve_aspect: bool,
) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(file);

    let image_reader = ImageReader::new(cursor).with_guessed_format()?;
    let decoded = image_reader.decode()?;

    let actual_width = dbg!(decoded.width());
    let actual_height = dbg!(decoded.height());

    let (width, height) = dims.unwrap_or((actual_width, actual_height));

    log::info!("{actual_width} != {width} && {actual_height} != {height}");
    let aspect = dbg!(actual_width as f64 / actual_height as f64);
    let width = width.min(actual_width);
    let height = if dbg!(preserve_aspect) {
        ((width as f64) / aspect) as u32
    } else {
        dbg!(height.min(actual_height))
    };

    let image_buf = resize(&decoded, width, height, FilterType::Lanczos3);

    let mut bytes = Vec::new();
    image_buf.write_to(&mut Cursor::new(&mut bytes), ImageFormat::WebP)?;

    Ok(bytes)
}
