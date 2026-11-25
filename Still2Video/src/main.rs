mod audio_vu;
mod exporter;
mod image_ops;
mod modal_crop;
mod modal_title;
mod models;
mod ui_main;

use eframe::egui;
use models::{
    load_session_from_folder, norm_rot, save_session_to_folder, scan_images_in_folder,
    ImageItem, SessionFile, SessionSettings, TitleScreenConfig,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct ExportProgress {
    pub active: bool,
    pub message: String,
    pub progress: f32,
}

pub struct Still2VideoApp {
    pub image_folder: Option<PathBuf>,
    pub image_data: Vec<ImageItem>,
    pub current_idx: usize,
    pub status_msg: String,

    // Video Configuration
    pub img_duration_str: String,
    pub trans_type: String,
    pub trans_dur_str: String,

    // Audio & Waveform
    pub music_path: Option<PathBuf>,
    pub audio_state: Arc<Mutex<audio_vu::AudioAnalysisState>>,
    pub vu_zoom: f32,

    // Modal Windows & Dialogs
    pub crop_editor: Option<modal_crop::CropEditorState>,
    pub title_editor: Option<modal_title::TitleEditorState>,
    pub duration_modal_idx: Option<usize>,
    pub duration_modal_str: String,
    pub title_screen: TitleScreenConfig,

    // GPU Texture Caches
    pub thumb_cache: HashMap<String, egui::TextureHandle>,
    pub preview_tex: Option<(String, egui::TextureHandle)>,
    pub drag_idx: Option<usize>,

    // Export Worker State
    pub export_progress: Arc<Mutex<ExportProgress>>,
}

impl Default for Still2VideoApp {
    fn default() -> Self {
        Self {
            image_folder: None,
            image_data: Vec::new(),
            current_idx: 0,
            status_msg: "No folder loaded.".into(),
            img_duration_str: "3.0".into(),
            trans_type: "Crossfade".into(),
            trans_dur_str: "1.0".into(),
            music_path: None,
            audio_state: Arc::new(Mutex::new(audio_vu::AudioAnalysisState::default())),
            vu_zoom: 1.0,
            crop_editor: None,
            title_editor: None,
            duration_modal_idx: None,
            duration_modal_str: String::new(),
            title_screen: TitleScreenConfig::default(),
            thumb_cache: HashMap::new(),
            preview_tex: None,
            drag_idx: None,
            export_progress: Arc::new(Mutex::new(ExportProgress {
                active: false,
                message: String::new(),
                progress: 0.0,
            })),
        }
    }
}

impl Still2VideoApp {
    pub fn parsed_img_dur(&self) -> f32 {
        self.img_duration_str.trim().parse::<f32>().unwrap_or(3.0).max(0.1)
    }

    pub fn parsed_trans_dur(&self) -> f32 {
        self.trans_dur_str.trim().parse::<f32>().unwrap_or(1.0).max(0.0)
    }

    pub fn effective_image_list(&self) -> Vec<ImageItem> {
        let mut items = self.image_data.clone();
        if self.title_screen.enabled {
            items.insert(
                0,
                ImageItem {
                    duration: Some(self.title_screen.duration),
                    is_title: true,
                    ..Default::default()
                },
            );
        }
        items
    }

    pub fn load_folder(&mut self, folder: PathBuf) {
        let found = scan_images_in_folder(&folder);
        if found.is_empty() {
            self.status_msg = "No valid images found in folder!".into();
            return;
        }
        self.image_folder = Some(folder.clone());
        if let Some(session) = load_session_from_folder(&folder) {
            let mut valid: Vec<ImageItem> = session
                .images
                .into_iter()
                .filter(|d| std::path::Path::new(&d.path).exists())
                .collect();
            for d in &mut valid {
                d.rotation = norm_rot(d.rotation);
            }
            self.image_data = if valid.is_empty() { found } else { valid };
            if let Some(d) = session.settings.image_duration { self.img_duration_str = d; }
            if let Some(t) = session.settings.transition_type { self.trans_type = t; }
            if let Some(td) = session.settings.transition_duration { self.trans_dur_str = td; }
            if let Some(ts) = session.settings.title_screen { self.title_screen = ts; }
            if let Some(mp) = session.settings.music_path {
                let p = PathBuf::from(&mp);
                if p.exists() {
                    self.set_music_track(p);
                }
            }
        } else {
            self.image_data = found;
        }
        self.thumb_cache.clear();
        self.preview_tex = None;
        self.current_idx = 0;
        self.status_msg = format!("Timeline: {} images.", self.image_data.len());
        self.save_session();
    }

    pub fn save_session(&self) {
        if let Some(folder) = &self.image_folder {
            let session = SessionFile {
                settings: SessionSettings {
                    image_duration: Some(self.img_duration_str.clone()),
                    transition_type: Some(self.trans_type.clone()),
                    transition_duration: Some(self.trans_dur_str.clone()),
                    music_path: self.music_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    title_screen: Some(self.title_screen.clone()),
                },
                images: self.image_data.clone(),
            };
            save_session_to_folder(folder, &session);
        }
    }
}

impl eframe::App for Still2VideoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);

        egui::TopBottomPanel::bottom("vu_panel").exact_height(135.0).show(ctx, |ui| {
            self.draw_waveform_panel(ui);
        });
        egui::TopBottomPanel::bottom("strip_panel").exact_height(125.0).show(ctx, |ui| {
            self.draw_thumbnail_strip(ui);
        });
        egui::SidePanel::left("left_panel").exact_width(275.0).show(ctx, |ui| {
            self.draw_left_panel(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_preview_panel(ui);
        });

        self.draw_modals(ctx);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1150.0, 920.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Still2Video — Image to Video Creator",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(Still2VideoApp::default()))
        }),
    )
}

