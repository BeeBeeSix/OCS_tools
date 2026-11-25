use crate::models::ImageItem;
use eframe::egui;
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba, RgbaImage};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};

/// Rotate an image counter-clockwise (matching PIL's `img.rotate(angle, expand=True)`).
pub fn rotate_img(img: DynamicImage, angle_deg: f32) -> DynamicImage {
    let angle = ((angle_deg % 360.0) + 360.0) % 360.0;
    if angle.abs() < 0.001 {
        return img;
    }
    if (angle - 90.0).abs() < 0.001 {
        return img.rotate270(); // PIL 90 CCW == image crate 270 CW
    }
    if (angle - 180.0).abs() < 0.001 {
        return img.rotate180();
    }
    if (angle - 270.0).abs() < 0.001 {
        return img.rotate90(); // PIL 270 CCW == image crate 90 CW
    }

    // Arbitrary angle with canvas expansion
    let rad = angle.to_radians();
    let (w, h) = img.dimensions();
    let (wf, hf) = (w as f32, h as f32);
    let new_w = (wf * rad.cos().abs() + hf * rad.sin().abs()).ceil() as u32;
    let new_h = (wf * rad.sin().abs() + hf * rad.cos().abs()).ceil() as u32;

    let mut expanded = RgbaImage::from_pixel(new_w, new_h, Rgba([0, 0, 0, 255]));
    let ox = ((new_w - w) / 2) as i64;
    let oy = ((new_h - h) / 2) as i64;
    image::imageops::overlay(&mut expanded, &img.to_rgba8(), ox, oy);

    // PIL rotates CCW for positive degrees; imageproc rotates CW for positive radians
    let rotated = rotate_about_center(
        &expanded,
        -rad,
        Interpolation::Bicubic,
        Rgba([0, 0, 0, 255]),
    );
    DynamicImage::ImageRgba8(rotated)
}

/// Apply rotation and normalized [nx, ny, nw, nh] crop to an image item.
pub fn load_processed_image(item: &ImageItem) -> Option<DynamicImage> {
    let mut img = image::open(&item.path).ok()?;
    img = rotate_img(img, item.rotation);
    if let Some([nx, ny, nw, nh]) = item.crop {
        let (iw, ih) = img.dimensions();
        let x = (nx * iw as f32).round().max(0.0) as u32;
        let y = (ny * ih as f32).round().max(0.0) as u32;
        let w = (nw * iw as f32).round().max(1.0).min((iw - x) as f32) as u32;
        let h = (nh * ih as f32).round().max(1.0).min((ih - y) as f32) as u32;
        img = img.crop_imm(x, y, w, h);
    }
    Some(img)
}

/// Compute largest centred 16:9 normalized crop box (mirrors `_compute_autocrop_169`).
pub fn compute_autocrop_169(item: &ImageItem) -> Option<[f32; 4]> {
    let img = image::open(&item.path).ok()?;
    let img = rotate_img(img, item.rotation);
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let [nx, ny, nw, nh] = item.crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);

    let (rw, rh) = (nw * iw, nh * ih);
    let (rx, ry) = (nx * iw, ny * ih);

    let mut tw = rw;
    let mut th = (tw * 9.0 / 16.0).round();
    if th > rh {
        th = rh;
        tw = (th * 16.0 / 9.0).round();
    }
    let x0 = rx + (rw - tw) / 2.0;
    let y0 = ry + (rh - th) / 2.0;
    Some([x0 / iw, y0 / ih, tw / iw, th / ih])
}

/// Generate a 100x70 letterboxed egui ColorImage for the timeline strip.
pub fn build_thumbnail_color_image(item: &ImageItem, tw: u32, th: u32) -> Option<egui::ColorImage> {
    let img = load_processed_image(item)?;
    let resized = img.resize(tw, th, FilterType::Lanczos3);
    let mut bg = RgbaImage::from_pixel(tw, th, Rgba([30, 30, 30, 255]));
    let ox = ((tw - resized.width()) / 2) as i64;
    let oy = ((th - resized.height()) / 2) as i64;
    image::imageops::overlay(&mut bg, &resized.to_rgba8(), ox, oy);
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [tw as usize, th as usize],
        bg.as_raw(),
    ))
}

/// Generate a preview ColorImage fitted inside max_w x max_h.
pub fn build_preview_color_image(item: &ImageItem, max_w: u32, max_h: u32) -> Option<egui::ColorImage> {
    let img = load_processed_image(item)?;
    let resized = img.resize(max_w.max(64), max_h.max(64), FilterType::Lanczos3).to_rgba8();
    let (w, h) = resized.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        resized.as_raw(),
    ))
}

/// Generate a 1920x1080 letterboxed RGB frame for final video export.
pub fn render_export_frame(item: &ImageItem, target_w: u32, target_h: u32) -> Option<image::RgbImage> {
    let img = load_processed_image(item)?;
    let resized = img.resize(target_w, target_h, FilterType::Lanczos3).to_rgb8();
    let mut bg = image::RgbImage::from_pixel(target_w, target_h, image::Rgb([0, 0, 0]));
    let ox = ((target_w - resized.width()) / 2) as i64;
    let oy = ((target_h - resized.height()) / 2) as i64;
    image::imageops::overlay(&mut bg, &resized, ox, oy);
    Some(bg)
}
