    pub fn ui(&mut self, ui: &mut egui::Ui, dem: Option<&DemData>, min_str: &mut String, max_str: &mut String) -> bool {
        let mut changed = false;
        ui.vertical(|ui| {
            ui.set_width(700.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(700.0, 520.0), Sense::click_and_drag());
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

                let img_rect = Rect::from_min_size(rect.min + self.pan, rect.size() * self.zoom);
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
