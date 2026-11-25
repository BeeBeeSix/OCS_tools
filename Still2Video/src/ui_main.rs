use crate::image_ops::{
    build_preview_color_image, build_thumbnail_color_image, compute_autocrop_169,
};
use crate::modal_crop::CropEditorState;
use crate::modal_title::TitleEditorState;
use crate::models::{calc_video_duration, fmt_time, norm_rot};
use crate::Still2VideoApp;
use eframe::egui;
use std::path::Path;

impl Still2VideoApp {
    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.crop_editor.is_some()
            || self.title_editor.is_some() || self.duration_modal_idx.is_some()
        {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowLeft) { self.kb_move(-1); }
            if i.key_pressed(egui::Key::ArrowRight) { self.kb_move(1); }
            if i.key_pressed(egui::Key::OpenBracket) { self.rotate_selected(90.0); }
            if i.key_pressed(egui::Key::CloseBracket) { self.rotate_selected(-90.0); }
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                self.delete_selected();
            }
            if i.key_pressed(egui::Key::D) { self.duplicate_selected(); }
            if i.key_pressed(egui::Key::C) { self.open_crop_modal(); }
            if i.key_pressed(egui::Key::F) {
                if i.modifiers.ctrl && i.modifiers.shift {
                    self.autocrop_169_all();
                } else {
                    self.autocrop_169();
                }
            }
        });
    }

    pub fn draw_left_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.strong("1. Select Image Source");
        if ui.add_sized([ui.available_width(), 24.0], egui::Button::new("Browse Folder")).clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                self.load_folder(folder);
            }
        }
        if ui.add_sized([ui.available_width(), 24.0], egui::Button::new("🎬 Title Screen…")).clicked() {
            self.title_editor = Some(TitleEditorState::new(&self.title_screen));
        }

        ui.add_space(10.0);
        ui.strong("2. Image Order");

        let list_h = (ui.available_height() - 310.0).max(110.0);
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(list_h).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for idx in 0..self.image_data.len() {
                    let item = &self.image_data[idx];
                    let name = Path::new(&item.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("image");
                    let rot_s = if item.rotation != 0.0 { format!(" ({}°)", item.rotation) } else { String::new() };
                    let dur_s = item.duration.map(|d| format!(" ⏱{:.2}s", d)).unwrap_or_default();
                    let label = format!("{}{}{}", name, rot_s, dur_s);

                    let selected = idx == self.current_idx;
                    if ui.selectable_label(selected, label).clicked() {
                        self.current_idx = idx;
                        self.preview_tex = None;
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            if ui.button("⟲ Rot L").clicked() { self.rotate_selected(90.0); }
            if ui.button("⟳ Rot R").clicked() { self.rotate_selected(-90.0); }
            if ui.button("✂ Crop").clicked() { self.open_crop_modal(); }
            if ui.button("❌ Del").clicked() { self.delete_selected(); }
        });
        if ui.add_sized([ui.available_width(), 22.0], egui::Button::new("✂✂ Auto Crop 16:9 — All Images")).clicked() {
            self.autocrop_169_all();
        }

        ui.add_space(10.0);
        ui.strong("3. Video Configuration");

        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Image Duration (sec):");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::TextEdit::singleline(&mut self.img_duration_str).desired_width(50.0)).changed() {
                    changed = true;
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Transition Effect:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("trans_combo")
                    .selected_text(&self.trans_type)
                    .width(115.0)
                    .show_ui(ui, |ui| {
                        for opt in ["None", "Fade In/Out", "Crossfade", "Blur Dissolve", "Ken Burns"] {
                            if ui.selectable_value(&mut self.trans_type, opt.to_string(), opt).changed() {
                                changed = true;
                            }
                        }
                    });
            });
        });

        ui.horizontal(|ui| {
            ui.label("Transition (sec):");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::TextEdit::singleline(&mut self.trans_dur_str).desired_width(50.0)).changed() {
                    changed = true;
                }
            });
        });
        if changed { self.save_session(); }

        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Est. Video Length:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let eff = self.effective_image_list();
                if eff.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(77, 166, 255), "—");
                } else {
                    let tot = calc_video_duration(&eff, self.parsed_img_dur(), self.parsed_trans_dur(), &self.trans_type);
                    ui.colored_label(
                        egui::Color32::from_rgb(77, 166, 255),
                        format!("{}  ({:.1}s)", fmt_time(tot), tot),
                    );
                }
            });
        });
        ui.separator();

        ui.strong("4. Music Track (optional)");
        ui.horizontal(|ui| {
            if let Some(mp) = &self.music_path {
                let fname = mp.file_name().and_then(|n| n.to_str()).unwrap_or("audio");
                ui.label(fname);
            } else {
                ui.colored_label(egui::Color32::RED, "No track selected.");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Browse").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Audio", &["mp3", "wav", "aac", "ogg", "flac"])
                        .pick_file()
                    {
                        self.set_music_track(p);
                        self.save_session();
                    }
                }
            });
        });
        if ui.add_sized([ui.available_width(), 22.0], egui::Button::new("✕ Clear Music")).clicked() {
            self.clear_music_track();
            self.save_session();
        }

        ui.add_space(10.0);
        if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("▶  Export Video")).clicked() {
            self.start_export_dialog();
        }
    }
    pub fn draw_preview_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Timeline Preview");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });	
        ui.add_space(4.0);

        let avail = ui.available_size();
        let canvas_h = (avail.y - 60.0).max(200.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(avail.x, canvas_h), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(45, 45, 45));

        if let Some(item) = self.image_data.get(self.current_idx) {
            let key = format!("{}_{}_{:?}_{}x{}", item.path, item.rotation, item.crop, rect.width() as u32, rect.height() as u32);
            let needs_reload = self.preview_tex.as_ref().map(|(k, _)| k != &key).unwrap_or(true);
            if needs_reload {
                if let Some(cimg) = build_preview_color_image(item, rect.width() as u32, rect.height() as u32) {
                    let tex = ui.ctx().load_texture(&key, cimg, egui::TextureOptions::LINEAR);
                    self.preview_tex = Some((key, tex));
                }
            }
            if let Some((_, tex)) = &self.preview_tex {
                let sz = tex.size_vec2();
                let img_rect = egui::Rect::from_center_size(rect.center(), sz);
                ui.painter().image(
                    tex.id(),
                    img_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 140.0).max(0.0) / 2.0);
            if ui.button("◀ Prev").clicked() && !self.image_data.is_empty() {
                let n = self.image_data.len();
                self.current_idx = (self.current_idx + n - 1) % n;
                self.preview_tex = None;
            }
            if ui.button("Next ▶").clicked() && !self.image_data.is_empty() {
                self.current_idx = (self.current_idx + 1) % self.image_data.len();
                self.preview_tex = None;
            }
        });
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(&self.status_msg).italics().size(11.0));
        });
    }

    pub fn draw_thumbnail_strip(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Timeline  (drag to reorder  |  right-click for options  |  [ ] rotate  ← → move  Del delete  D duplicate)")
                .strong()
                .size(11.5),
        );

        let thumb_w = 100.0;
        let thumb_h = 70.0;
        let pad = 6.0;

        egui::ScrollArea::horizontal().id_salt("thumb_strip_scroll").show(ui, |ui| {
            let n = self.image_data.len();
            let total_w = (n as f32 * (thumb_w + pad) + pad).max(ui.available_width());
            let (strip_rect, response) = ui.allocate_exact_size(
                egui::vec2(total_w, thumb_h + pad * 2.0 + 18.0),
                egui::Sense::click_and_drag(),
            );
            let painter = ui.painter_at(strip_rect);
            painter.rect_filled(strip_rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

            // Drag & click selection
            if response.drag_started() || response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let rel_x = pos.x - strip_rect.left() - pad;
                    let idx = (rel_x / (thumb_w + pad)).floor() as isize;
                    if idx >= 0 && (idx as usize) < n {
                        self.current_idx = idx as usize;
                        self.preview_tex = None;
                        if response.drag_started() {
                            self.drag_idx = Some(idx as usize);
                        }
                    }
                }
            }
            if response.dragged() {
                if let (Some(from), Some(pos)) = (self.drag_idx, response.interact_pointer_pos()) {
                    let rel_x = pos.x - strip_rect.left() - pad;
                    let target = (rel_x / (thumb_w + pad)).floor() as isize;
                    if target >= 0 && (target as usize) < n && (target as usize) != from {
                        self.image_data.swap(from, target as usize);
                        self.current_idx = target as usize;
                        self.drag_idx = Some(target as usize);
                        self.save_session();
                    }
                }
            }
            if response.drag_stopped() {
                self.drag_idx = None;
            }

            // Right-click context menu
            response.context_menu(|ui| {
                if ui.button("🎬 Edit Title Screen…").clicked() {
                    self.title_editor = Some(TitleEditorState::new(&self.title_screen));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("⟲  Rotate Left  ([)").clicked() { self.rotate_selected(90.0); ui.close_menu(); }
                if ui.button("⟳  Rotate Right  (])").clicked() { self.rotate_selected(-90.0); ui.close_menu(); }
                if ui.button("✂  Crop / Zoom  (C)").clicked() { self.open_crop_modal(); ui.close_menu(); }
                if ui.button("✂  Auto Crop 16:9  (F)").clicked() { self.autocrop_169(); ui.close_menu(); }
                if ui.button("⏱  Set Duration…").clicked() {
                    if let Some(it) = self.image_data.get(self.current_idx) {
                        self.duration_modal_idx = Some(self.current_idx);
                        self.duration_modal_str = it.duration.map(|d| format!("{}", d)).unwrap_or_default();
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("◀  Move Left  (←)").clicked() { self.kb_move(-1); ui.close_menu(); }
                if ui.button("▶  Move Right  (→)").clicked() { self.kb_move(1); ui.close_menu(); }
                if ui.button("❐  Duplicate  (D)").clicked() { self.duplicate_selected(); ui.close_menu(); }
                if ui.button("✕  Delete  (Del)").clicked() { self.delete_selected(); ui.close_menu(); }
            });

            // Draw thumbnails
            for i in 0..n {
                let x0 = strip_rect.left() + pad + i as f32 * (thumb_w + pad);
                let y0 = strip_rect.top() + pad;
                let t_rect = egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(thumb_w, thumb_h));

                if i == self.current_idx {
                    painter.rect_stroke(
                        t_rect.expand(2.5),
                        0.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(77, 166, 255)),
                    );
                }

                let item = &self.image_data[i];
                let key = format!("thumb_{}_{}_{:?}", item.path, item.rotation, item.crop);
                if !self.thumb_cache.contains_key(&key) {
                    if let Some(cimg) = build_thumbnail_color_image(item, 100, 70) {
                        let tex = ui.ctx().load_texture(&key, cimg, egui::TextureOptions::LINEAR);
                        self.thumb_cache.insert(key.clone(), tex);
                    }
                }
                if let Some(tex) = self.thumb_cache.get(&key) {
                    painter.image(
                        tex.id(),
                        t_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }

                let fname = Path::new(&item.path).file_name().and_then(|s| s.to_str()).unwrap_or("");
                let short: String = if fname.chars().count() > 13 {
                    format!("{}…", fname.chars().take(11).collect::<String>())
                } else {
                    fname.to_string()
                };
                painter.text(
                    egui::pos2(x0 + thumb_w / 2.0, y0 + thumb_h + 2.0),
                    egui::Align2::CENTER_TOP,
                    short,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_rgb(170, 170, 170),
                );
            }
        });
    }

    pub fn kb_move(&mut self, dir: isize) {
        if self.image_data.is_empty() { return; }
        let idx = self.current_idx as isize;
        let next = idx + dir;
        if next >= 0 && (next as usize) < self.image_data.len() {
            self.image_data.swap(idx as usize, next as usize);
            self.current_idx = next as usize;
            self.save_session();
        }
    }

    pub fn rotate_selected(&mut self, angle: f32) {
        if let Some(item) = self.image_data.get_mut(self.current_idx) {
            item.rotation = norm_rot(item.rotation + angle);
            self.preview_tex = None;
            self.save_session();
        }
    }

    pub fn delete_selected(&mut self) {
        if self.image_data.is_empty() { return; }
        self.image_data.remove(self.current_idx);
        if self.current_idx >= self.image_data.len() && !self.image_data.is_empty() {
            self.current_idx = self.image_data.len() - 1;
        }
        self.preview_tex = None;
        self.save_session();
    }

    pub fn duplicate_selected(&mut self) {
        if let Some(item) = self.image_data.get(self.current_idx).cloned() {
            self.image_data.insert(self.current_idx + 1, item);
            self.current_idx += 1;
            self.save_session();
        }
    }

    pub fn open_crop_modal(&mut self) {
        if let Some(item) = self.image_data.get(self.current_idx) {
            self.crop_editor = Some(CropEditorState::new(self.current_idx, item));
        }
    }

    pub fn autocrop_169(&mut self) {
        if let Some(item) = self.image_data.get_mut(self.current_idx) {
            if let Some(crop) = compute_autocrop_169(item) {
                item.crop = Some(crop);
                self.preview_tex = None;
                self.save_session();
            }
        }
    }

    pub fn autocrop_169_all(&mut self) {
        let mut count = 0;
        for item in &mut self.image_data {
            if let Some(crop) = compute_autocrop_169(item) {
                item.crop = Some(crop);
                count += 1;
            }
        }
        self.preview_tex = None;
        self.status_msg = format!("Auto-cropped {} of {} images to 16:9.", count, self.image_data.len());
        self.save_session();
    }
}
