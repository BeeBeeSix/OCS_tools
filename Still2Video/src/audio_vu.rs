use crate::models::{calc_video_duration, fmt_time, segment_times};
use crate::Still2VideoApp;
use eframe::egui;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Default)]
pub struct AudioAnalysisState {
    pub rms: Vec<f32>,
    pub duration: f32,
    pub peak_db: f32,
    pub status: String,
    pub is_loading: bool,
}

impl Still2VideoApp {
    pub fn set_music_track(&mut self, path: PathBuf) {
        self.music_path = Some(path.clone());
        let state_arc = Arc::clone(&self.audio_state);
        {
            let mut st = state_arc.lock().unwrap();
            st.rms.clear();
            st.duration = 0.0;
            st.status = "Analysing audio…".into();
            st.is_loading = true;
        }
        std::thread::spawn(move || {
            analyse_audio_worker(path, state_arc);
        });
    }

    pub fn clear_music_track(&mut self) {
        self.music_path = None;
        let mut st = self.audio_state.lock().unwrap();
        st.rms.clear();
        st.duration = 0.0;
        st.status.clear();
        st.is_loading = false;
    }
}

fn analyse_audio_worker(path: PathBuf, state_arc: Arc<Mutex<AudioAnalysisState>>) {
    let res = decode_and_compute_rms(&path);
    let mut st = state_arc.lock().unwrap();
    st.is_loading = false;
    match res {
        Ok((rms, dur, peak_db)) => {
            st.rms = rms;
            st.duration = dur;
            st.peak_db = peak_db;
            st.status.clear();
        }
        Err(err) => {
            st.status = format!("Error: {}", err);
        }
    }
}

fn decode_and_compute_rms(path: &PathBuf) -> Result<(Vec<f32>, f32, f32), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("No audio track found")?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100) as f32;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1).max(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    let mut mono_samples: Vec<f32> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id { continue; }
        if let Ok(decoded) = decoder.decode(&packet) {
            let spec = *decoded.spec();
            let buf = sample_buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
            buf.copy_interleaved_ref(decoded);
            for frame in buf.samples().chunks(channels) {
                let sum: f32 = frame.iter().sum();
                mono_samples.push(sum / channels as f32);
            }
        }
    }

    if mono_samples.is_empty() {
        return Err("Decoded 0 samples".into());
    }

    let duration = mono_samples.len() as f32 / sample_rate;
    let peak_lin = mono_samples.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
    let peak_db = 20.0 * (peak_lin + 1e-9).log10();

    let n_buckets = 1200;
    let chunk = (mono_samples.len() / n_buckets).max(1);
    let mut rms: Vec<f32> = mono_samples
        .chunks(chunk)
        .map(|c| (c.iter().map(|&s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    let max_rms = rms.iter().fold(0.0_f32, |m, &v| m.max(v));
    if max_rms > 0.0 {
        for v in &mut rms { *v /= max_rms; }
    }

    Ok((rms, duration, peak_db))
}
impl Still2VideoApp {
    pub fn draw_waveform_panel(&mut self, ui: &mut egui::Ui) {
        let (rms, audio_dur, peak_db, status_txt) = {
            let st = self.audio_state.lock().unwrap();
            (st.rms.clone(), st.duration, st.peak_db, st.status.clone())
        };

        // Header bar with Zoom Controls & Peak dBFS
        ui.horizontal(|ui| {
            ui.strong("Audio Waveform  —  time-aligned to video");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !rms.is_empty() {
                    let col = if peak_db > -3.0 {
                        egui::Color32::from_rgb(255, 68, 68)
                    } else {
                        egui::Color32::from_rgb(136, 136, 136)
                    };
                    ui.colored_label(col, format!("Peak: {:.1} dBFS", peak_db));
                }
                if ui.small_button("↺").clicked() { self.vu_zoom = 1.0; }
                if ui.small_button("+").clicked() { self.vu_zoom = (self.vu_zoom * 1.5).min(32.0); }
                ui.label(format!("{:.1}×", self.vu_zoom));
                if ui.small_button("−").clicked() { self.vu_zoom = (self.vu_zoom / 1.5).max(0.25); }
            });
        });

        let eff = self.effective_image_list();
        let n = eff.len();
        let img_dur = self.parsed_img_dur();
        let trans_dur = self.parsed_trans_dur();
        let video_dur = if n > 0 {
            calc_video_duration(&eff, img_dur, trans_dur, &self.trans_type)
        } else {
            audio_dur
        };
        let segs = if n > 0 {
            segment_times(&eff, img_dur, trans_dur, &self.trans_type)
        } else {
            Vec::new()
        };

        let vu_h = 88.0;
        let avail_w = ui.available_width();

        egui::ScrollArea::horizontal().id_salt("vu_scroll").show(ui, |ui| {
            let ref_dur = video_dur.max(audio_dur).max(1.0);
            let base_pps = (avail_w / ref_dur).max(80.0);
            let px_per_sec = base_pps * self.vu_zoom;
            let canvas_w = (ref_dur * px_per_sec + 40.0).max(avail_w);

            let (rect, resp) = ui.allocate_exact_size(egui::vec2(canvas_w, vu_h), egui::Sense::hover());
            if resp.hovered() {
                ui.input(|i| {
                    if i.modifiers.ctrl && i.raw_scroll_delta.y != 0.0 {
                        let factor = if i.raw_scroll_delta.y > 0.0 { 1.2 } else { 1.0 / 1.2 };
                        self.vu_zoom = (self.vu_zoom * factor).clamp(0.25, 32.0);
                    }
                });
            }

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(14, 14, 14));
            let cy = rect.top() + vu_h * 0.5;

            if rms.is_empty() && n == 0 {
                let msg = if status_txt.is_empty() { "No audio loaded" } else { &status_txt };
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, msg,
                    egui::FontId::proportional(12.0), egui::Color32::from_rgb(90, 90, 90));
                return;
            }

            // RMS Waveform Bars
            if !rms.is_empty() && audio_dur > 0.0 {
                let bar_w = ((audio_dur / rms.len() as f32) * px_per_sec).max(1.0);
                let thresh = 10.0_f32.powf(-3.0 / 20.0);
                for (i, &v) in rms.iter().enumerate() {
                    let x = rect.left() + i as f32 * bar_w;
                    let amp = v * (vu_h * 0.44);
                    let col = if v >= thresh {
                        egui::Color32::from_rgb(255, 68, 68)
                    } else {
                        egui::Color32::from_rgb(0, 204, 102)
                    };
                    let bar_rect = egui::Rect::from_min_max(
                        egui::pos2(x, cy - amp),
                        egui::pos2(x + (bar_w - 0.5).max(1.0), cy + amp),
                    );
                    painter.rect_filled(bar_rect, 0.0, col);
                }
            }

            // Image Segment Boundaries & Transition Shading
            let blue = egui::Color32::from_rgb(77, 166, 255);
            let gold = egui::Color32::from_rgb(255, 204, 0);
            for (i, &(s_start, s_end)) in segs.iter().enumerate() {
                let xs = rect.left() + s_start * px_per_sec;
                let is_title = eff.get(i).map(|it| it.is_title).unwrap_or(false);
                let col = if is_title { gold } else { blue };
                painter.line_segment(
                    [egui::pos2(xs, rect.top()), egui::pos2(xs, rect.bottom() - 14.0)],
                    egui::Stroke::new(1.0, col),
                );
                let lbl = if is_title { "T".into() } else { format!("#{}", i) };
                painter.text(egui::pos2(xs + 3.0, rect.top() + 2.0), egui::Align2::LEFT_TOP,
                    lbl, egui::FontId::proportional(9.0), col);

                if i + 1 < segs.len() {
                    let next_s = segs[i + 1].0;
                    let (zs, ze) = if next_s < s_end { (next_s, s_end) } else { (s_end, next_s) };
                    if ze > zs {
                        let z_rect = egui::Rect::from_min_max(
                            egui::pos2(rect.left() + zs * px_per_sec, rect.top() + 2.0),
                            egui::pos2(rect.left() + ze * px_per_sec, rect.bottom() - 14.0),
                        );
                        painter.rect_filled(z_rect, 0.0, egui::Color32::from_white_alpha(28));
                    }
                }
            }

            // Video End Marker & Fade-out Band
            if video_dur > 0.0 {
                let xve = rect.left() + video_dur * px_per_sec;
                painter.line_segment([egui::pos2(xve, rect.top()), egui::pos2(xve, rect.bottom())],
                    egui::Stroke::new(2.0, gold));
                painter.text(egui::pos2(xve + 3.0, rect.top() + 2.0), egui::Align2::LEFT_TOP,
                    fmt_time(video_dur), egui::FontId::proportional(10.0), gold);

                if !rms.is_empty() {
                    let fade_dur = 3.0_f32.min(video_dur / 2.0);
                    let f_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + (video_dur - fade_dur) * px_per_sec, rect.top() + 2.0),
                        egui::pos2(xve, rect.bottom() - 14.0),
                    );
                    painter.rect_filled(f_rect, 0.0, egui::Color32::from_rgba_unmultiplied(255, 136, 0, 35));
                }
            }
        });

        // Bottom info row
        ui.horizontal(|ui| {
            if audio_dur > 0.0 {
                ui.label(egui::RichText::new(format!("Audio: {} ({:.1}s)", fmt_time(audio_dur), audio_dur)).size(10.5));
            } else {
                ui.label(egui::RichText::new("—").size(10.5));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(egui::Color32::from_rgb(0, 204, 102), "▎audio");
                ui.colored_label(egui::Color32::from_rgb(130, 130, 130), "▎transition");
                ui.colored_label(egui::Color32::from_rgb(77, 166, 255), "▎image");
            });
        });
    }
}
