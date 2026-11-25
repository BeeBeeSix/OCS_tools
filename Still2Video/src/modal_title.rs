use crate::models::TitleScreenConfig;
use crate::Still2VideoApp;
use ab_glyph::{FontArc, PxScale};
use eframe::egui;
use image::{imageops::FilterType, Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut, text_size};
use imageproc::rect::Rect;
use std::fs;

pub struct TitleEditorState {
    pub cfg: TitleScreenConfig,
    pub preview_tex: Option<egui::TextureHandle>,
    pub dirty: bool,
}

impl TitleEditorState {
    pub fn new(existing: &TitleScreenConfig) -> Self {
        let mut cfg = existing.clone();
        if !cfg.enabled && cfg.title.is_empty() {
            cfg.enabled = true;
        }
        Self { cfg, preview_tex: None, dirty: true }
    }
}

fn parse_hex_rgb(hex: &str) -> Rgb<u8> {
    let h = hex.trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
        return Rgb([r, g, b]);
    }
        Rgb::from([255, 255, 255])
}

fn color_hex_picker(ui: &mut egui::Ui, hex: &mut String) -> bool {
    let rgb = parse_hex_rgb(hex);
    let [r, g, b] = rgb.0;
    let mut c32 = egui::Color32::from_rgb(r, g, b);
    if ui.color_edit_button_srgba(&mut c32).changed() {
        *hex = format!("#{:02x}{:02x}{:02x}", c32.r(), c32.g(), c32.b());
        return true;
    }
    false
}

fn load_system_font(face: &str, bold: bool) -> Option<FontArc> {
    let f = face.to_lowercase();
    let suffix = if bold { "bd" } else { "" };
    let candidates = [
        format!("C:\\Windows\\Fonts\\{}{}.ttf", f, suffix),
        format!("C:\\Windows\\Fonts\\{}.ttf", f),
        if bold { "C:\\Windows\\Fonts\\arialbd.ttf".into() } else { "C:\\Windows\\Fonts\\arial.ttf".into() },
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into(),
    ];
    for path in candidates {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(font) = FontArc::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }
    None
}
pub fn render_title_frame(base_img_path: Option<&str>, ts: &TitleScreenConfig, w: u32, h: u32) -> RgbImage {
    let mut bg = if ts.bg_mode == "solid" {
        RgbImage::from_pixel(w, h, parse_hex_rgb(&ts.solid_color))
    } else if let Some(p) = base_img_path.and_then(|p| image::open(p).ok()) {
        let src = p.to_rgb8();
        let (sw, sh) = (src.width() as f32, src.height() as f32);
        let (nw, nh) = if sw / sh > w as f32 / h as f32 {
            ((h as f32 * (sw / sh)).round() as u32, h)
        } else {
            (w, (w as f32 / (sw / sh)).round() as u32)
        };
        let scaled = image::imageops::resize(&src, nw.max(w), nh.max(h), FilterType::Lanczos3);
        let x0 = (scaled.width() - w) / 2;
        let y0 = (scaled.height() - h) / 2;
        image::imageops::crop_imm(&scaled, x0, y0, w, h).to_image()
    } else {
        RgbImage::from_pixel(w, h, Rgb::from([0, 0, 0]))
    };

    if ts.bg_mode.contains("blur") && ts.blur_radius > 0.0 {
        let scale_f = w as f32 / 1920.0;
        bg = image::imageops::blur(&bg, (ts.blur_radius * scale_f).max(0.5));
    }
    if ts.bg_mode.contains("darken") {
        let f = ts.darken_factor.clamp(0.0, 1.0);
        for px in bg.pixels_mut() {
            for ch in px.0.iter_mut() {
                *ch = (*ch as f32 * f) as u8;
            }
        }
    }

    let scale_ratio = h as f32 / 1080.0;
    let lines = [
        (&ts.title, ts.title_size * scale_ratio, parse_hex_rgb(&ts.title_color), true, true),
        (&ts.subtitle, ts.subtitle_size * scale_ratio, parse_hex_rgb(&ts.subtitle_color), false, false),
        (&ts.date, ts.date_size * scale_ratio, parse_hex_rgb(&ts.date_color), false, false),
    ];

    struct LineSpec<'a> { text: &'a str, font: FontArc, scale: PxScale, col: Rgb<u8>, tw: i32, th: i32, is_title: bool }
    let mut active = Vec::new();
    for (txt, sz, col, bold, is_title) in lines {
        if txt.trim().is_empty() { continue; }
        if let Some(font) = load_system_font(&ts.font_face, bold) {
            let sc = PxScale::from(sz.max(8.0));
            let (tw, th) = text_size(sc, &font, txt);
            active.push(LineSpec { text: txt, font, scale: sc, col, tw: tw as i32, th: th as i32, is_title });
        }
    }
    if active.is_empty() { return bg; }

    let pad = (ts.line_padding * scale_ratio) as i32;
    let r_gap = (ts.rule_gap * scale_ratio) as i32;
    let r_thick = (ts.rule_thickness * scale_ratio).max(1.0) as i32;
    let mut total_h: i32 = active.iter().map(|l| l.th).sum::<i32>() + pad * (active.len() as i32 - 1);
    if ts.show_rule && active.len() > 1 { total_h += r_gap * 2 + r_thick; }

    let margin = (w as f32 * 0.05) as i32;
    let mut y = ((h as f32 * ts.v_position) as i32 - total_h / 2).clamp(16, (h as i32 - total_h - 16).max(16));

    for (idx, l) in active.iter().enumerate() {
        let x = match ts.text_align.as_str() {
            "left" => margin,
            "right" => (w as i32 - l.tw - margin).max(0),
            _ => ((w as i32 - l.tw) / 2).max(0),
        };
        if ts.shadow {
            let off = (4.0 * scale_ratio).max(1.0) as i32;
            draw_text_mut(&mut bg, Rgb::from([0, 0, 0]), x + off, y + off, l.scale, &l.font, l.text);
        }
        draw_text_mut(&mut bg, l.col, x, y, l.scale, &l.font, l.text);
        y += l.th;

        if ts.show_rule && l.is_title && idx + 1 < active.len() {
            y += r_gap;
            let rw = (w as f32 * ts.rule_width) as i32;
            let rx = match ts.text_align.as_str() {
                "left" => margin,
                "right" => (w as i32 - rw - margin).max(0),
                _ => ((w as i32 - rw) / 2).max(0),
            };
            draw_filled_rect_mut(&mut bg, Rect::at(rx, y).of_size(rw.max(1) as u32, r_thick as u32), parse_hex_rgb(&ts.rule_color));
            y += r_thick + r_gap;
        } else {
            y += pad;
        }
    }
    bg
}
impl Still2VideoApp {
    pub fn draw_modals(&mut self, ctx: &egui::Context) {
        self.draw_crop_modal(ctx);
        self.draw_title_modal(ctx);
        self.draw_duration_modal(ctx);
        self.draw_export_modal(ctx);
    }

    pub fn draw_title_modal(&mut self, ctx: &egui::Context) {
        let mut close_win = false;
        let mut apply_ts = None;
        let mut clear_ts = false;
        let base_path = self.image_data.first().map(|it| it.path.clone());

        if let Some(ed) = &mut self.title_editor {
            egui::Window::new("🎬 Title Screen Editor")
                .collapsible(false)
                .resizable(false)
                .fixed_size([980.0, 640.0])
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(320.0);
                            ui.set_min_height(570.0);
                            egui::ScrollArea::vertical()
                                .id_salt("title_controls_scroll")
                                .max_height(570.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let c = &mut ed.cfg;
                                    if ui.checkbox(&mut c.enabled, "Enable title screen").changed() { ed.dirty = true; }
                                    ui.horizontal(|ui| {
                                        ui.label("Duration (sec):");
                                        if ui.add(egui::DragValue::new(&mut c.duration).range(1.0..=30.0).speed(0.5)).changed() { ed.dirty = true; }
                                    });
                                    ui.separator();
                                    ui.strong("Text Content");
                                    for (lbl, txt, sz, col) in [
                                        ("Title", &mut c.title, &mut c.title_size, &mut c.title_color),
                                        ("Subtitle", &mut c.subtitle, &mut c.subtitle_size, &mut c.subtitle_color),
                                        ("Date / caption", &mut c.date, &mut c.date_size, &mut c.date_color),
                                    ] {
                                        ui.label(egui::RichText::new(lbl).size(11.0).weak());
                                        ui.horizontal(|ui| {
                                            if ui.add(egui::TextEdit::singleline(txt).desired_width(195.0)).changed() { ed.dirty = true; }
                                            if ui.add(egui::DragValue::new(sz).range(12.0..=200.0)).changed() { ed.dirty = true; }
                                            if color_hex_picker(ui, col) { ed.dirty = true; }
                                        });
                                    }
                                    ui.separator();
                                    ui.strong("Typography");
                                    ui.horizontal(|ui| {
                                        ui.label("Font:");
                                        egui::ComboBox::from_id_salt("ts_font").selected_text(&c.font_face).width(140.0).show_ui(ui, |ui| {
                                            for f in ["arial", "times", "courier", "verdana", "georgia", "trebuchet", "impact"] {
                                                if ui.selectable_value(&mut c.font_face, f.into(), f).changed() { ed.dirty = true; }
                                            }
                                        });
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Alignment:");
                                        egui::ComboBox::from_id_salt("ts_align").selected_text(&c.text_align).width(100.0).show_ui(ui, |ui| {
                                            for a in ["center", "left", "right"] {
                                                if ui.selectable_value(&mut c.text_align, a.into(), a).changed() { ed.dirty = true; }
                                            }
                                        });
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Line padding:");
                                        if ui.add(egui::DragValue::new(&mut c.line_padding).range(0.0..=80.0)).changed() { ed.dirty = true; }
                                    });
                                    if ui.add(egui::Slider::new(&mut c.v_position, 0.1..=0.9).text("Vertical pos")).changed() { ed.dirty = true; }
                                    if ui.checkbox(&mut c.shadow, "Drop shadow").changed() { ed.dirty = true; }
                                    ui.separator();
                                    ui.strong("Horizontal Rule (under title)");
                                    if ui.checkbox(&mut c.show_rule, "Show rule").changed() { ed.dirty = true; }
                                    ui.horizontal(|ui| {
                                        ui.label("Colour:");
                                        if color_hex_picker(ui, &mut c.rule_color) { ed.dirty = true; }
                                        ui.label(egui::RichText::new(&c.rule_color).size(11.0).weak());
                                    });
                                    if ui.add(egui::Slider::new(&mut c.rule_width, 0.1..=1.0).text("Width (0–1)")).changed() { ed.dirty = true; }
                                    ui.horizontal(|ui| {
                                        ui.label("Thickness (px):");
                                        if ui.add(egui::DragValue::new(&mut c.rule_thickness).range(1.0..=20.0)).changed() { ed.dirty = true; }
                                        ui.label("Gap (px):");
                                        if ui.add(egui::DragValue::new(&mut c.rule_gap).range(0.0..=60.0)).changed() { ed.dirty = true; }
                                    });
                                    ui.separator();
                                    ui.strong("Background");
                                    ui.horizontal(|ui| {
                                        ui.label("Mode:");
                                        egui::ComboBox::from_id_salt("ts_bg").selected_text(&c.bg_mode).width(130.0).show_ui(ui, |ui| {
                                            for m in ["blur+darken", "blur", "darken", "solid", "original"] {
                                                if ui.selectable_value(&mut c.bg_mode, m.into(), m).changed() { ed.dirty = true; }
                                            }
                                        });
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Blur radius:");
                                        if ui.add(egui::DragValue::new(&mut c.blur_radius).range(0.0..=60.0)).changed() { ed.dirty = true; }
                                    });
                                    if ui.add(egui::Slider::new(&mut c.darken_factor, 0.0..=1.0).text("Darken (0–1)")).changed() { ed.dirty = true; }
                                    ui.horizontal(|ui| {
                                        ui.label("Solid colour:");
                                        if color_hex_picker(ui, &mut c.solid_color) { ed.dirty = true; }
                                    });
                                });
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.strong("Preview  (live — 16:9)");
                            if ed.dirty {
                                let rgb = render_title_frame(base_path.as_deref(), &ed.cfg, 640, 360);
                                let size = [640_usize, 360_usize];
                                let cimg = egui::ColorImage::from_rgb(size, rgb.as_raw());
                                ed.preview_tex = Some(ctx.load_texture("title_prev", cimg, egui::TextureOptions::LINEAR));
                                ed.dirty = false;
                            }
                            let (prev_rect, _) = ui.allocate_exact_size(egui::vec2(620.0, 350.0), egui::Sense::hover());
                            ui.painter().rect_filled(prev_rect, 0.0, egui::Color32::BLACK);
                            if let Some(tex) = &ed.preview_tex {
                                ui.painter().image(tex.id(), prev_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                            }
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("🗑 Clear Title").clicked() { clear_ts = true; close_win = true; }
                        if ui.button("🔄 Refresh Preview").clicked() { ed.dirty = true; }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✅ Apply").clicked() { apply_ts = Some(ed.cfg.clone()); close_win = true; }
                            if ui.button("❌ Cancel").clicked() { close_win = true; }
                        });
                    });
                });
        }
        if let Some(cfg) = apply_ts { self.title_screen = cfg; self.save_session(); }
        if clear_ts { self.title_screen.enabled = false; self.save_session(); }
        if close_win { self.title_editor = None; }
    }

    fn draw_duration_modal(&mut self, ctx: &egui::Context) {
        let Some(idx) = self.duration_modal_idx else { return; };
        let mut close = false;
        egui::Window::new("Slide Duration").collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("Global default: {:.2}s (leave blank for default)", self.parsed_img_dur()));
                ui.text_edit_singleline(&mut self.duration_modal_str);
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() { self.duration_modal_str.clear(); }
                    if ui.button("✔ Apply").clicked() {
                        if let Some(it) = self.image_data.get_mut(idx) {
                            let s = self.duration_modal_str.trim();
                            it.duration = if s.is_empty() { None } else { s.parse::<f32>().ok().filter(|&v| v > 0.0) };
                            self.save_session();
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() { close = true; }
                });
            });
        if close { self.duration_modal_idx = None; }
    }
}
