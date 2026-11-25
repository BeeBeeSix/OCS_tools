use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ImageItem {
    pub path: String,
    #[serde(default)]
    pub rotation: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f32; 4]>, // [nx, ny, nw, nh]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f32>,
    #[serde(skip)]
    pub is_title: bool,
}

impl ImageItem {
    pub fn effective_dur(&self, default_dur: f32) -> f32 {
        self.duration.unwrap_or(default_dur)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TitleScreenConfig {
    pub enabled: bool,
    pub title: String,
    pub subtitle: String,
    pub date: String,
    pub title_size: f32,
    pub subtitle_size: f32,
    pub date_size: f32,
    pub title_color: String,
    pub subtitle_color: String,
    pub date_color: String,
    pub font_face: String,
    pub text_align: String,
    pub v_position: f32,
    pub bg_mode: String,
    pub blur_radius: f32,
    pub darken_factor: f32,
    pub solid_color: String,
    pub shadow: bool,
    pub duration: f32,
    pub line_padding: f32,
    pub show_rule: bool,
    pub rule_color: String,
    pub rule_width: f32,
    pub rule_thickness: f32,
    pub rule_gap: f32,
}

impl Default for TitleScreenConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            title: String::new(),
            subtitle: String::new(),
            date: String::new(),
            title_size: 96.0,
            subtitle_size: 48.0,
            date_size: 36.0,
            title_color: "#ffffff".into(),
            subtitle_color: "#cccccc".into(),
            date_color: "#aaaaaa".into(),
            font_face: "arial".into(),
            text_align: "center".into(),
            v_position: 0.5,
            bg_mode: "blur+darken".into(),
            blur_radius: 20.0,
            darken_factor: 0.5,
            solid_color: "#1a1a2e".into(),
            shadow: true,
            duration: 4.0,
            line_padding: 20.0,
            show_rule: false,
            rule_color: "#ffffff".into(),
            rule_width: 0.6,
            rule_thickness: 3.0,
            rule_gap: 18.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SessionSettings {
    pub image_duration: Option<String>,
    pub transition_type: Option<String>,
    pub transition_duration: Option<String>,
    pub music_path: Option<String>,
    pub title_screen: Option<TitleScreenConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SessionFile {
    pub settings: SessionSettings,
    pub images: Vec<ImageItem>,
}

pub fn norm_rot(v: f32) -> f32 {
    let mut m = (v % 360.0 + 360.0) % 360.0;
    m = (m * 100.0).round() / 100.0;
    if (m - 360.0).abs() < 0.001 { 0.0 } else { m }
}

pub fn fmt_time(s: f32) -> String {
    let sec = s.round().max(0.0) as u32;
    format!("{}:{:02}", sec / 60, sec % 60)
}

pub fn calc_video_duration(items: &[ImageItem], img_dur: f32, trans_dur: f32, trans_type: &str) -> f32 {
    let n = items.len();
    if n == 0 { return 0.0; }
    let durs: Vec<f32> = items.iter().map(|it| it.effective_dur(img_dur)).collect();
    let mut total = 0.0;
    for i in 0..n {
        total += durs[i];
        if i + 1 < n {
            let eff_safe = trans_dur.min(durs[i] / 2.0).min(durs[i + 1] / 2.0);
            match trans_type {
                "Crossfade" | "Ken Burns" => total -= eff_safe,
                "Blur Dissolve" => total += eff_safe,
                _ => {}
            }
        }
    }
    total.max(0.0)
}

pub fn segment_times(items: &[ImageItem], img_dur: f32, trans_dur: f32, trans_type: &str) -> Vec<(f32, f32)> {
    let n = items.len();
    let durs: Vec<f32> = items.iter().map(|it| it.effective_dur(img_dur)).collect();
    let mut segs = Vec::with_capacity(n);
    let mut cursor = 0.0;
    for i in 0..n {
        segs.push((cursor, cursor + durs[i]));
        if i + 1 < n {
            let eff_safe = trans_dur.min(durs[i] / 2.0).min(durs[i + 1] / 2.0);
            match trans_type {
                "Crossfade" | "Ken Burns" => cursor += durs[i] - eff_safe,
                "Blur Dissolve" => cursor += durs[i] + eff_safe,
                _ => cursor += durs[i],
            }
        }
    }
    segs
}

pub fn load_session_from_folder(folder: &Path) -> Option<SessionFile> {
    let sf = folder.join(".still2video_session.json");
    let text = fs::read_to_string(sf).ok()?;
    if let Ok(session) = serde_json::from_str::<SessionFile>(&text) {
        return Some(session);
    }
    // Support legacy plain-list JSON format
    let images = serde_json::from_str::<Vec<ImageItem>>(&text).ok()?;
    Some(SessionFile { settings: SessionSettings::default(), images })
}

pub fn save_session_to_folder(folder: &Path, session: &SessionFile) {
    let sf = folder.join(".still2video_session.json");
    if let Ok(json) = serde_json::to_string_pretty(session) {
        let _ = fs::write(sf, json);
    }
}

pub fn scan_images_in_folder(folder: &PathBuf) -> Vec<ImageItem> {
    let exts = ["png", "jpg", "jpeg", "bmp", "tiff"];
    let mut files: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file() && p.extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files.into_iter()
        .map(|p| ImageItem { path: p.to_string_lossy().into_owned(), ..Default::default() })
        .collect()
}
