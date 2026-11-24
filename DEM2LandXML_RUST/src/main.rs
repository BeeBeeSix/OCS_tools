#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dem;
mod landxml;
mod preview;
mod theme;
mod ui;

use eframe::egui;
use ui::App;

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1090.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
    "DEM to LandXML / CSV (Rust)",
    opts,
    Box::new(|cc| {
        let app = App::default();
        app.theme.apply(&cc.egui_ctx);
        Ok(Box::new(app))
    }),
	)
}
