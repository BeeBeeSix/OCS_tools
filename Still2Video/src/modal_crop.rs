use crate::image_ops::rotate_img;
use crate::models::{norm_rot, ImageItem};
use eframe::egui;
use image::imageops::FilterType;

#[derive(Clone, Copy, PartialEq)]
pub enum DragMode {
    Draw,
    Move,
    Handle(usize), // 0:nw, 1:n, 2:ne, 3:w, 4:e, 5:sw, 6:s, 7:se
}

pub struct CropEditorState {
    pub item_idx: usize,
    pub path: String,
    pub base_rot: f32,
    pub custom_rot: f32,
    pub keep_169: bool,
    pub rx0: f32,
    pub ry0: f32,
    pub rx1: f32,
    pub ry1: f32,
    pub drag_mode: Option<DragMode>,
    pub drag_start_pos: egui::Pos2,
    pub drag_start_rect: [f32; 4],
    pub src_w: f32,
    pub src_h: f32,
    pub disp_w: f32,
    pub disp_h: f32,
    pub scale: f32,
    pub tex: Option<egui::TextureHandle>,
    pub dirty_tex: bool,
}

impl CropEditorState {
    pub fn new(idx: usize, item: &ImageItem) -> Self {
        let base = (item.rotation / 90.0).round() * 90.0;
        let fine = ((item.rotation - base) * 100.0).round() / 100.0;
        let (sw, sh) = image::image_dimensions(&item.path).unwrap_or((1920, 1080));
        let [nx, ny, nw, nh] = item.crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let ratio = (nw * sw as f32) / (nh * sh as f32).max(1.0);
        let is_169 = item.crop.is_none() || ((ratio - (16.0 / 9.0)).abs() / (16.0 / 9.0) < 0.02);

        Self {
            item_idx: idx,
            path: item.path.clone(),
            base_rot: norm_rot(base),
            custom_rot: fine.clamp(-45.0, 45.0),
            keep_169: is_169,
            rx0: nx,
            ry0: ny,
            rx1: nx + nw,
            ry1: ny + nh,
            drag_mode: None,
            drag_start_pos: egui::Pos2::ZERO,
            drag_start_rect: [0.0; 4],
            src_w: sw as f32,
            src_h: sh as f32,
            disp_w: 640.0,
            disp_h: 480.0,
            scale: 1.0,
            tex: None,
            dirty_tex: true,
        }
    }

    pub fn rebuild_texture(&mut self, ctx: &egui::Context) {
        if let Ok(img) = image::open(&self.path) {
            let rot = rotate_img(img, self.base_rot + self.custom_rot);
            let (ow, oh) = (rot.width() as f32, rot.height() as f32);
            self.scale = (700.0 / ow).min(500.0 / oh).min(1.0);
            self.disp_w = (ow * self.scale).round().max(1.0);
            self.disp_h = (oh * self.scale).round().max(1.0);
            let resized = rot.resize(self.disp_w as u32, self.disp_h as u32, FilterType::Lanczos3).to_rgba8();
            let cimg = egui::ColorImage::from_rgba_unmultiplied(
                [resized.width() as usize, resized.height() as usize],
                resized.as_raw(),
            );
            self.tex = Some(ctx.load_texture("crop_preview", cimg, egui::TextureOptions::LINEAR));
        }
        self.dirty_tex = false;
        if self.keep_169 {
            self.fit_169();
        }
    }

    pub fn fit_169(&mut self) {
        let target = 16.0_f32 / 9.0;
        let (bw, bh) = if (self.base_rot as i32) % 180 != 0 {
            (self.src_h, self.src_w)
        } else {
            (self.src_w, self.src_h)
        };
        let (w0, h0) = (bw * self.scale, bh * self.scale);
        let th = self.custom_rot.to_radians();
        let (c, sn) = (th.cos().abs(), th.sin().abs());
        let m = if th.abs() < 1e-6 { 0.0 } else { 2.0 };
        let b = ((w0 / 2.0 - m) / (target * c + sn))
            .min((h0 / 2.0 - m) / (target * sn + c))
            .max(1.0);
        let a = target * b;
        let hw = (a / self.disp_w).min(0.5);
        let hh = (b / self.disp_h).min(0.5);
        self.rx0 = 0.5 - hw;
        self.rx1 = 0.5 + hw;
        self.ry0 = 0.5 - hh;
        self.ry1 = 0.5 + hh;
    }
}
use crate::Still2VideoApp;

impl Still2VideoApp {
    pub fn draw_crop_modal(&mut self, ctx: &egui::Context) {
        let mut close_modal = false;
        let mut apply_changes = false;

        if let Some(ed) = &mut self.crop_editor {
            if ed.dirty_tex {
                ed.rebuild_texture(ctx);
            }

            egui::Window::new("Crop / Rotate — drag to select region")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("Drag to draw crop  |  handles to resize  |  drag inside to move  |  scroll to zoom")
                            .italics()
                            .size(11.0),
                    );

                    let (area_rect, _) = ui.allocate_exact_size(egui::vec2(700.0, 500.0), egui::Sense::hover());
                    ui.painter().rect_filled(area_rect, 0.0, egui::Color32::from_rgb(25, 25, 25));

                    let img_rect = egui::Rect::from_center_size(area_rect.center(), egui::vec2(ed.disp_w, ed.disp_h));
                    let resp = ui.interact(img_rect, ui.id().with("crop_canvas"), egui::Sense::click_and_drag());

                    if let Some(tex) = &ed.tex {
                        ui.painter().image(
                            tex.id(),
                            img_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }

                    let cx0 = img_rect.left() + ed.rx0 * ed.disp_w;
                    let cy0 = img_rect.top() + ed.ry0 * ed.disp_h;
                    let cx1 = img_rect.left() + ed.rx1 * ed.disp_w;
                    let cy1 = img_rect.top() + ed.ry1 * ed.disp_h;
                    let crop_rect = egui::Rect::from_min_max(egui::pos2(cx0, cy0), egui::pos2(cx1, cy1));

                    let handles = [
                        egui::pos2(cx0, cy0), egui::pos2((cx0 + cx1) * 0.5, cy0), egui::pos2(cx1, cy0),
                        egui::pos2(cx0, (cy0 + cy1) * 0.5),                       egui::pos2(cx1, (cy0 + cy1) * 0.5),
                        egui::pos2(cx0, cy1), egui::pos2((cx0 + cx1) * 0.5, cy1), egui::pos2(cx1, cy1),
                    ];

                    if resp.drag_started() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            ed.drag_start_pos = pos;
                            ed.drag_start_rect = [ed.rx0, ed.ry0, ed.rx1, ed.ry1];
                            if let Some((hi, _)) = handles.iter().enumerate().find(|(_, h)| h.distance(pos) <= 10.0) {
                                ed.drag_mode = Some(DragMode::Handle(hi));
                            } else if crop_rect.contains(pos) {
                                ed.drag_mode = Some(DragMode::Move);
                            } else {
                                ed.drag_mode = Some(DragMode::Draw);
                                ed.rx0 = ((pos.x - img_rect.left()) / ed.disp_w).clamp(0.0, 1.0);
                                ed.ry0 = ((pos.y - img_rect.top()) / ed.disp_h).clamp(0.0, 1.0);
                                ed.rx1 = ed.rx0;
                                ed.ry1 = ed.ry0;
                            }
                        }
                    }

                    if resp.dragged() {
                        if let (Some(mode), Some(pos)) = (ed.drag_mode, resp.interact_pointer_pos()) {
                            let nx = ((pos.x - img_rect.left()) / ed.disp_w).clamp(0.0, 1.0);
                            let ny = ((pos.y - img_rect.top()) / ed.disp_h).clamp(0.0, 1.0);
                            match mode {
                                DragMode::Draw => { ed.rx1 = nx; ed.ry1 = ny; }
                                 DragMode::Move => {
                                    let dx = (pos.x - ed.drag_start_pos.x) / ed.disp_w;
                                    let dy = (pos.y - ed.drag_start_pos.y) / ed.disp_h;
                                    let [sx0, sy0, sx1, sy1] = ed.drag_start_rect;
                                    let w = sx1 - sx0;
                                    let h = sy1 - sy0;
                                    ed.rx0 = (sx0 + dx).clamp(0.0, 1.0 - w);
                                    ed.ry0 = (sy0 + dy).clamp(0.0, 1.0 - h);
                                    ed.rx1 = ed.rx0 + w;
                                    ed.ry1 = ed.ry0 + h;
                                }
                                DragMode::Handle(hi) => {
                                    if matches!(hi, 0 | 3 | 5) { ed.rx0 = nx.min(ed.rx1 - 0.02); }
                                    if matches!(hi, 2 | 4 | 7) { ed.rx1 = nx.max(ed.rx0 + 0.02); }
                                    if matches!(hi, 0 | 1 | 2) { ed.ry0 = ny.min(ed.ry1 - 0.02); }
                                    if matches!(hi, 5 | 6 | 7) { ed.ry1 = ny.max(ed.ry0 + 0.02); }
                                }
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        if ed.rx0 > ed.rx1 { std::mem::swap(&mut ed.rx0, &mut ed.rx1); }
                        if ed.ry0 > ed.ry1 { std::mem::swap(&mut ed.ry0, &mut ed.ry1); }
                        if ed.rx1 - ed.rx0 < 0.02 || ed.ry1 - ed.ry0 < 0.02 {
                            ed.rx0 = 0.0; ed.ry0 = 0.0; ed.rx1 = 1.0; ed.ry1 = 1.0;
                        }
                        ed.drag_mode = None;
                    }

                    if resp.hovered() {
                        ui.input(|i| {
                            if i.raw_scroll_delta.y != 0.0 {
                                let f = if i.raw_scroll_delta.y > 0.0 { 0.92 } else { 1.08 };
                                let cx = (ed.rx0 + ed.rx1) * 0.5;
                                let cy = (ed.ry0 + ed.ry1) * 0.5;
                                let hw = ((ed.rx1 - ed.rx0) * 0.5 * f).clamp(0.03, 0.5);
                                let hh = ((ed.ry1 - ed.ry0) * 0.5 * f).clamp(0.03, 0.5);
                                ed.rx0 = (cx - hw).clamp(0.0, 1.0 - hw * 2.0);
                                ed.ry0 = (cy - hh).clamp(0.0, 1.0 - hh * 2.0);
                                ed.rx1 = ed.rx0 + hw * 2.0;
                                ed.ry1 = ed.ry0 + hh * 2.0;
                            }
                        });
                    }

                    let p = ui.painter();
                    let dim = egui::Color32::from_black_alpha(130);
                    p.rect_filled(egui::Rect::from_min_max(img_rect.min, egui::pos2(img_rect.max.x, cy0)), 0.0, dim);
                    p.rect_filled(egui::Rect::from_min_max(egui::pos2(img_rect.min.x, cy1), img_rect.max), 0.0, dim);
                    p.rect_filled(egui::Rect::from_min_max(egui::pos2(img_rect.min.x, cy0), egui::pos2(cx0, cy1)), 0.0, dim);
                    p.rect_filled(egui::Rect::from_min_max(egui::pos2(cx1, cy0), egui::pos2(img_rect.max.x, cy1)), 0.0, dim);

                    let pw = ((ed.rx1 - ed.rx0) * ed.disp_w / ed.scale).round();
                    let ph = ((ed.ry1 - ed.ry0) * ed.disp_h / ed.scale).round();
                    let ratio = pw / ph.max(1.0);
                    let is_169 = ((ratio - 16.0 / 9.0).abs() / (16.0 / 9.0)) < 0.005;
                    let col = if is_169 { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::from_rgb(77, 166, 255) };

                    p.rect_stroke(crop_rect, 0.0, egui::Stroke::new(2.0, col));
                    for h in handles {
                        p.rect_filled(egui::Rect::from_center_size(h, egui::vec2(10.0, 10.0)), 1.0, col);
                    }
                    p.text(
                        egui::pos2(cx0 + 5.0, cy1 - 5.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{}×{}px  {:.3}:1{}", pw, ph, ratio, if is_169 { "  ✓ 16:9" } else { "" }),
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );

                    ui.horizontal(|ui| {
                        ui.label("Custom Rotation:");
                        if ui.add(egui::Slider::new(&mut ed.custom_rot, -15.0..=15.0).step_by(0.1).suffix("°")).changed() {
                            ed.dirty_tex = true;
                        }
                        if ui.button("-90°").clicked() { ed.base_rot = norm_rot(ed.base_rot - 90.0); ed.dirty_tex = true; }
                        if ui.button("+90°").clicked() { ed.base_rot = norm_rot(ed.base_rot + 90.0); ed.dirty_tex = true; }
                        if ui.button("↺ 0°").clicked() { ed.custom_rot = 0.0; ed.dirty_tex = true; }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("↺ Reset All").clicked() {
                            ed.base_rot = 0.0; ed.custom_rot = 0.0;
                            ed.rx0 = 0.0; ed.ry0 = 0.0; ed.rx1 = 1.0; ed.ry1 = 1.0;
                            ed.dirty_tex = true;
                        }
                        if ui.button("⬛ Fit 16:9").clicked() { ed.fit_169(); }
                        ui.checkbox(&mut ed.keep_169, "Keep 16:9 on rotate");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Cancel").clicked() { close_modal = true; }
                            if ui.button("✔ Apply").clicked() { apply_changes = true; close_modal = true; }
                        });
                    });
                });
        }

        if apply_changes {
            if let Some(ed) = &self.crop_editor {
                if let Some(item) = self.image_data.get_mut(ed.item_idx) {
                    item.rotation = norm_rot(ed.base_rot + ed.custom_rot);
                    if ed.rx0 <= 0.001 && ed.ry0 <= 0.001 && ed.rx1 >= 0.999 && ed.ry1 >= 0.999 {
                        item.crop = None;
                    } else {
                        item.crop = Some([ed.rx0, ed.ry0, ed.rx1 - ed.rx0, ed.ry1 - ed.ry0]);
                    }
                    self.preview_tex = None;
                    self.save_session();
                }
            }
        }
        if close_modal {
            self.crop_editor = None;
        }
    }
}
