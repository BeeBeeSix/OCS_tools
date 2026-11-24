use crate::dem::DemData;
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};

pub struct DemPreview {
    pub texture: Option<TextureHandle>,
    pub zoom: f32,
    pub pan: Vec2,
    pub poly_geo: Vec<(f64, f64)>,
    pub cursor_text: String,
    tex_step: usize,
}

impl Default for DemPreview {
    fn default() -> Self {
        Self {
            texture: None, zoom: 1.0, pan: Vec2::ZERO, poly_geo: Vec::new(),
            cursor_text: "L-Click: Poly | R-Click: Min Z | Shift+R-Click: Max Z".into(),
            tex_step: 1,
        }
    }
}

impl DemPreview {
    pub fn rebuild_texture(&mut self, ctx: &egui::Context, dem: &DemData, min_z: Option<f32>, max_z: Option<f32>) {
        self.tex_step = (dem.width.max(dem.height) / 900).max(1);
        let (tw, th) = (dem.width.div_ceil(self.tex_step), dem.height.div_ceil(self.tex_step));
        let (mut vmin, mut vmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for r in (0..dem.height).step_by(self.tex_step) {
            for c in (0..dem.width).step_by(self.tex_step) {
                let z = dem.data[r * dem.width + c];
                if dem.is_valid(z, min_z, max_z) { vmin = vmin.min(z); vmax = vmax.max(z); }
            }
        }
        let lo = min_z.unwrap_or(vmin);
        let hi = max_z.unwrap_or(vmax).max(lo + 0.01);
        let mut pixels = vec![Color32::from_rgb(30, 30, 30); tw * th];
        for (tr, r) in (0..dem.height).step_by(self.tex_step).enumerate() {
            for (tc, c) in (0..dem.width).step_by(self.tex_step).enumerate() {
                let z = dem.data[r * dem.width + c];
                if dem.is_valid(z, min_z, max_z) {
                    let t = ((z - lo) / (hi - lo)).clamp(0.0, 1.0);
                    pixels[tr * tw + tc] = terrain_color(t);
                }
            }
        }
        let img = ColorImage { size: [tw, th], pixels };
        self.texture = Some(ctx.load_texture("dem", img, egui::TextureOptions::LINEAR));
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, dem: Option<&DemData>, min_str: &mut String, max_str: &mut String) -> bool {
        let mut changed = false;
        let avail = ui.available_size();
        let canvas_w = (avail.x - 12.0).max(300.0);
        let canvas_h = (avail.y - 36.0).max(250.0);

        ui.vertical(|ui| {
            ui.set_width(canvas_w);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(25, 25, 25));


            if let (Some(tex), Some(d)) = (&self.texture, dem) {
                if resp.hovered() {
                    let scroll = ui.input(|i| i.raw_scroll_delta.y);
                    if scroll != 0.0 {
                        if let Some(pos) = resp.hover_pos() {
                            let factor = if scroll > 0.0 { 1.2 } else { 0.83 };
                            let new_z = (self.zoom * factor).clamp(1.0, 25.0);
                            let rel = pos - rect.min - self.pan;
                            self.pan += rel * (1.0 - new_z / self.zoom);
                            self.zoom = new_z;
                        }
                    }
                }
                if resp.dragged_by(egui::PointerButton::Middle) { self.pan += resp.drag_delta(); }

            // Fit DEM inside canvas while preserving true 1:1 X/Y world aspect ratio
            let (min_x, min_y, max_x, max_y) = d.bounds();
            let geo_aspect = ((max_x - min_x) / (max_y - min_y).max(1e-6)) as f32;
            let (fit_w, fit_h) = if rect.width() / rect.height() > geo_aspect {
                (rect.height() * geo_aspect, rect.height())
            } else {
                (rect.width(), rect.width() / geo_aspect)
            };
            let base_offset = Vec2::new((rect.width() - fit_w) * 0.5, (rect.height() - fit_h) * 0.5);
            let img_rect = Rect::from_min_size(
                rect.min + base_offset + self.pan,
                Vec2::new(fit_w, fit_h) * self.zoom,
            );

            ui.painter().with_clip_rect(rect).image(
                tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE,
            );


                let (min_x, min_y, max_x, max_y) = d.bounds();
                let to_geo = |p: Pos2| {
                    let u = ((p.x - img_rect.min.x) / img_rect.width()).clamp(0.0, 1.0) as f64;
                    let v = ((p.y - img_rect.min.y) / img_rect.height()).clamp(0.0, 1.0) as f64;
                    (min_x + u * (max_x - min_x), max_y - v * (max_y - min_y))
                };
                let to_scr = |(gx, gy): (f64, f64)| {
                    let u = ((gx - min_x) / (max_x - min_x)) as f32;
                    let v = ((max_y - gy) / (max_y - min_y)) as f32;
                    Pos2::new(img_rect.min.x + u * img_rect.width(), img_rect.min.y + v * img_rect.height())
                };

                if let Some(pos) = resp.hover_pos() {
                    let (gx, gy) = to_geo(pos);
                    let col = (((gx - min_x) / d.pixel_x) as usize).min(d.width - 1);
                    let row = (((max_y - gy) / d.pixel_y) as usize).min(d.height - 1);
                    let z = d.data[row * d.width + col];
                    let valid = d.is_valid(z, None, None);
                    self.cursor_text = if !valid { format!("E: {gx:.1} | N: {gy:.1} | Z: NoData") }
                        else { format!("E: {gx:.1} | N: {gy:.1} | Z: {z:.2} m") };
                    if resp.clicked() { self.poly_geo.push((gx, gy)); changed = true; }
                    if resp.secondary_clicked() && valid {
                        if ui.input(|i| i.modifiers.shift) { *max_str = format!("{z:.2}"); } else { *min_str = format!("{z:.2}"); }
                        changed = true;
                    }
                }

                let pts: Vec<Pos2> = self.poly_geo.iter().map(|&p| to_scr(p)).collect();
                let p_clip = ui.painter().with_clip_rect(rect);
                if pts.len() >= 3 { p_clip.add(egui::Shape::closed_line(pts.clone(), Stroke::new(2.0, Color32::RED))); }
                else if pts.len() == 2 { p_clip.add(egui::Shape::line(pts.clone(), Stroke::new(2.0, Color32::RED))); }
                for pt in pts { p_clip.circle_filled(pt, 3.5, Color32::YELLOW); }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 150, 255), &self.cursor_text);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Set Poly to View").clicked() {
                            self.poly_geo = vec![to_geo(rect.left_top()), to_geo(rect.right_top()), to_geo(rect.right_bottom()), to_geo(rect.left_bottom())];
                            changed = true;
                        }
                        if ui.button("Reset View").clicked() { self.zoom = 1.0; self.pan = Vec2::ZERO; }
                        if ui.button("Clear Poly").clicked() { self.poly_geo.clear(); changed = true; }
                    });
                });
            }
        });
        changed
    }
}

fn terrain_color(t: f32) -> Color32 {
    let (r, g, b) = if t < 0.35 { (40.0 + t * 180.0, 130.0 + t * 240.0, 70.0) }
    else if t < 0.75 { (100.0 + (t - 0.35) * 320.0, 200.0 - (t - 0.35) * 120.0, 60.0) }
    else { (230.0 + (t - 0.75) * 100.0, 155.0 + (t - 0.75) * 380.0, 120.0 + (t - 0.75) * 520.0) };
    Color32::from_rgb(r as u8, g as u8, b as u8)
}
