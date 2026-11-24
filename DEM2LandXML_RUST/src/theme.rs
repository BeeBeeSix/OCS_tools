use eframe::egui::{self, Color32, Visuals};

#[derive(Clone, Copy, PartialEq)]
pub enum AppTheme {
    Dark,
    Light,
    SlateBlue,
    HighContrast,
}

impl AppTheme {
    pub const ALL: [AppTheme; 4] = [
        AppTheme::Dark,
        AppTheme::Light,
        AppTheme::SlateBlue,
        AppTheme::HighContrast,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark (Default)",
            Self::Light => "Light",
            Self::SlateBlue => "Slate Blue",
            Self::HighContrast => "High Contrast",
        }
    }

    pub fn apply(self, ctx: &egui::Context) {
        let mut vis = match self {
            Self::Light => Visuals::light(),
            _ => Visuals::dark(),
        };
        match self {
            Self::SlateBlue => {
                vis.panel_fill = Color32::from_rgb(24, 34, 48);
                vis.window_fill = Color32::from_rgb(24, 34, 48);
                vis.extreme_bg_color = Color32::from_rgb(15, 22, 32);
                vis.widgets.inactive.bg_fill = Color32::from_rgb(38, 54, 76);
                vis.widgets.hovered.bg_fill = Color32::from_rgb(52, 76, 108);
            }
            Self::HighContrast => {
                vis.panel_fill = Color32::BLACK;
                vis.extreme_bg_color = Color32::from_rgb(10, 10, 10);
                vis.widgets.inactive.fg_stroke.color = Color32::WHITE;
                vis.widgets.inactive.bg_stroke.color = Color32::from_rgb(0, 220, 255);
                vis.widgets.inactive.bg_stroke.width = 1.5;
            }
            _ => {}
        }
        ctx.set_visuals(vis);
    }
}
