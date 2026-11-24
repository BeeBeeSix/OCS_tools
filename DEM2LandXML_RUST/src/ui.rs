use crate::dem::{load_boundary, DemData};
use crate::landxml::{export_dem, ExportMode, Tin12daOptions};
use crate::preview::DemPreview;
use crate::theme::AppTheme;
use eframe::egui;
use std::path::PathBuf;

pub struct App {
    pub in_path: String,
    pub out_path: String,
    pub surf_name: String,
    pub step: usize,
    pub mode: ExportMode,
    pub tin_name: String,
    pub tin_colour: String,
    pub tin_null_length: String,
    pub theme: AppTheme,
    pub min_z_str: String,
    pub max_z_str: String,
    pub est_points: usize,
    pub status: String,
    pub dem: Option<DemData>,
    pub preview: DemPreview,
}

impl Default for App {
    fn default() -> Self {
        Self {
            in_path: String::new(),
            out_path: String::new(),
            surf_name: "DEM_Surface".into(),
            step: 1,
            mode: ExportMode::Both,
            tin_name: String::new(),
            tin_colour: "green".into(),
            tin_null_length: String::new(),
            theme: AppTheme::Light,
            min_z_str: String::new(),
            max_z_str: String::new(),
            est_points: 0,
            status: "Ready.".into(),
            dem: None,
            preview: DemPreview::default(),
        }
    }
}

impl App {
    fn parse_z(s: &str) -> Option<f32> {
        s.trim().parse::<f32>().ok()
    }

    fn parse_f64(s: &str) -> Option<f64> {
        s.trim().parse::<f64>().ok()
    }

    fn default_ext(&self) -> &'static str {
        match self.mode {
            ExportMode::CsvOnly => "csv",
            ExportMode::Tin12daOnly => "12da",
            ExportMode::Both | ExportMode::XmlOnly | ExportMode::All => "xml",
        }
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        let (min_z, max_z) = (Self::parse_z(&self.min_z_str), Self::parse_z(&self.max_z_str));
        if let Some(d) = &self.dem {
            self.preview.rebuild_texture(ctx, d, min_z, max_z);
            self.est_points = d.estimate_count(self.step.max(1), &self.preview.poly_geo, min_z, max_z);
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(340.0);
                    ui.horizontal(|ui| {
                        ui.label("UI Theme:");
                        egui::ComboBox::from_id_salt("theme_box")
                            .selected_text(self.theme.label())
                            .show_ui(ui, |ui| {
                                for t in AppTheme::ALL {
                                    if ui.selectable_value(&mut self.theme, t, t.label()).clicked() {
                                        self.theme.apply(ctx);
                                    }
                                }
                            });
                    });
                    ui.add_space(4.0);
                    ui.label("Input GeoTIFF DEM:");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.in_path);
                        if ui.button("Browse").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("GeoTIFF", &["tif", "tiff"]).pick_file() {
                                self.in_path = p.display().to_string();
                                self.out_path = p.with_extension(self.default_ext()).display().to_string();
                                match DemData::load(&p) {
                                    Ok(d) => { self.dem = Some(d); self.preview.poly_geo.clear(); self.refresh(ctx); }
                                    Err(e) => self.status = format!("Error: {e}"),
                                }
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.label("Output File Path:");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.out_path);
                        if ui.button("Save As").clicked() {
                            if let Some(p) = rfd::FileDialog::new().save_file() { self.out_path = p.display().to_string(); }
                        }
                    });
                    ui.add_space(8.0);
                    ui.group(|ui| {
                        ui.label("Settings & Boundary");
                        ui.horizontal(|ui| { ui.label("Surface Name:"); ui.text_edit_singleline(&mut self.surf_name); });
                        ui.horizontal(|ui| {
                            ui.label("Subsample Step:");
                            if ui.add(egui::DragValue::new(&mut self.step).range(1..=200)).changed() { self.refresh(ctx); }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Format:");
                            let changed = ui.selectable_value(&mut self.mode, ExportMode::Both, "XML+CSV").clicked()
                                || ui.selectable_value(&mut self.mode, ExportMode::XmlOnly, "XML").clicked()
                                || ui.selectable_value(&mut self.mode, ExportMode::CsvOnly, "CSV").clicked()
                                || ui.selectable_value(&mut self.mode, ExportMode::Tin12daOnly, "12da").clicked()
                                || ui.selectable_value(&mut self.mode, ExportMode::All, "All").clicked();
                            if changed && !self.out_path.is_empty() {
                                self.out_path = PathBuf::from(&self.out_path).with_extension(self.default_ext()).display().to_string();
                            }
                        });
                        if matches!(self.mode, ExportMode::Tin12daOnly | ExportMode::All) {
                            ui.separator();
                            ui.label("12da TIN Options");
                            ui.horizontal(|ui| {
                                ui.label("TIN Name:");
                                ui.add(egui::TextEdit::singleline(&mut self.tin_name).hint_text("Defaults to Surface Name"));
                            });
                            ui.horizontal(|ui| {
                                ui.label("TIN Colour:");
                                ui.add(egui::TextEdit::singleline(&mut self.tin_colour).desired_width(100.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Null Length:");
                                ui.add(egui::TextEdit::singleline(&mut self.tin_null_length).desired_width(80.0).hint_text("Optional (m)"));
                            });
                            ui.separator();
                        }
                        ui.label(format!("Est. Export Points: {}", self.est_points));
                        if ui.button("Import Boundary (.shp / .geojson)").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Boundary", &["shp", "geojson", "json"]).pick_file() {
                                match load_boundary(&p) {
                                    Ok(pts) => { self.preview.poly_geo = pts; self.refresh(ctx); }
                                    Err(e) => self.status = format!("Boundary error: {e}"),
                                }
                            }
                        }
                        ui.label(format!("Crop Vertices: {}", self.preview.poly_geo.len()));
                    });
                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.label("Elevation Filter (Z)");
                        ui.horizontal(|ui| {
                            ui.label("Min:");
                            let c1 = ui.add(egui::TextEdit::singleline(&mut self.min_z_str).desired_width(60.0)).changed();
                            ui.label("Max:");
                            let c2 = ui.add(egui::TextEdit::singleline(&mut self.max_z_str).desired_width(60.0)).changed();
                            if ui.button("Clear").clicked() { self.min_z_str.clear(); self.max_z_str.clear(); self.refresh(ctx); }
                            if c1 || c2 { self.refresh(ctx); }
                        });
                    });
                    ui.add_space(10.0);
                    if ui.add_sized([340.0, 36.0], egui::Button::new("Export Surface / Points")).clicked() {
                        if let Some(d) = &self.dem {
                            let tin_opts = Tin12daOptions {
                                tin_name: Some(self.tin_name.clone()),
                                colour: Some(self.tin_colour.clone()),
                                null_length: Self::parse_f64(&self.tin_null_length),
                            };
                            let res = export_dem(
                                d, &PathBuf::from(&self.out_path), &self.surf_name, self.step.max(1),
                                &self.preview.poly_geo, Self::parse_z(&self.min_z_str), Self::parse_z(&self.max_z_str),
                                self.mode, Some(&tin_opts),
                            );
                            self.status = match res {
                                Ok(n) => format!("Exported {n} points successfully!"),
                                Err(e) => format!("Failed: {e}"),
                            };
                        }
                    }
                    ui.add_space(6.0);
                    ui.label(&self.status);
                });
                ui.group(|ui| {
                    if self.preview.ui(ui, self.dem.as_ref(), &mut self.min_z_str, &mut self.max_z_str) {
                        self.refresh(ctx);
                    }
                });
            });
        });
    }
}
