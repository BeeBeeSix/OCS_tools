use crate::image_ops::render_export_frame;
use crate::modal_title::render_title_frame;
use crate::models::{ImageItem, TitleScreenConfig};
use crate::{ExportProgress, Still2VideoApp};
use eframe::egui;
use image::{imageops::FilterType, RgbImage};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

impl Still2VideoApp {
    pub fn start_export_dialog(&mut self) {
        if self.image_data.is_empty() {
            self.status_msg = "Timeline is empty!".into();
            return;
        }
        let Some(save_path) = rfd::FileDialog::new()
            .add_filter("MP4 Video", &["mp4"])
            .set_file_name("output.mp4")
            .save_file()
        else {
            return;
        };

        let items = self.image_data.clone();
        let title_cfg = self.title_screen.clone();
        let img_dur = self.parsed_img_dur();
        let trans_dur = self.parsed_trans_dur();
        let trans_type = self.trans_type.clone();
        let music_path = self.music_path.clone();
        let prog_arc = Arc::clone(&self.export_progress);

        {
            let mut p = prog_arc.lock().unwrap();
            p.active = true;
            p.message = "Preparing frames…".into();
            p.progress = 0.0;
        }

        std::thread::spawn(move || {
            let res = run_export_pipeline(
                save_path.clone(),
                items,
                title_cfg,
                img_dur,
                trans_dur,
                trans_type,
                music_path,
                Arc::clone(&prog_arc),
            );
            let mut p = prog_arc.lock().unwrap();
            p.active = false;
            p.message = match res {
                Ok(_) => format!("Export complete: {}", save_path.display()),
                Err(e) => format!("Export failed: {}", e),
            };
        });
    }

    pub fn draw_export_modal(&mut self, ctx: &egui::Context) {
        let (active, msg, prog) = {
            let p = self.export_progress.lock().unwrap();
            (p.active, p.message.clone(), p.progress)
        };
        if !active {
            if !msg.is_empty() && msg.starts_with("Export ") {
                self.status_msg = msg;
            }
            return;
        }
        ctx.request_repaint();
        egui::Window::new("Exporting Video")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(&msg);
                ui.add_space(6.0);
                ui.add(egui::ProgressBar::new(prog).show_percentage());
            });
    }
}

struct PreparedSlide {
    frame: RgbImage,
    duration: f32,
    kb_dx: f32,
    kb_dy: f32,
}

fn blend_rgb(a: &RgbImage, b: &RgbImage, alpha: f32) -> RgbImage {
    let t = alpha.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    let mut out = RgbImage::new(a.width(), a.height());
    for (o, (pa, pb)) in out.as_mut().iter_mut()
        .zip(a.as_raw().iter().zip(b.as_raw().iter()))
    {
        *o = (*pa as f32 * inv + *pb as f32 * t) as u8;
    }
    out
}


fn scale_brightness(img: &RgbImage, factor: f32) -> RgbImage {
    let f = factor.clamp(0.0, 1.0);
    let mut out = img.clone();
    for b in out.as_mut() {
        *b = (*b as f32 * f) as u8;
    }
    out
}

fn sample_slide(s: &PreparedSlide, local_t: f32, ken_burns: bool) -> RgbImage {
    if !ken_burns {
        return s.frame.clone();
    }
    let (w, h) = (s.frame.width() as f32, s.frame.height() as f32);
    let progress = (local_t / s.duration.max(0.001)).clamp(0.0, 1.0);
    let scale = 1.08 - 0.08 * progress;
    let cw = (w / scale).round().min(w) as u32;
    let ch = (h / scale).round().min(h) as u32;
    let cx = (w * 0.5 + s.kb_dx * w * progress).round() as i32;
    let cy = (h * 0.5 + s.kb_dy * h * progress).round() as i32;
    let x0 = (cx - cw as i32 / 2).clamp(0, (w as i32 - cw as i32).max(0)) as u32;
    let y0 = (cy - ch as i32 / 2).clamp(0, (h as i32 - ch as i32).max(0)) as u32;
    let cropped = image::imageops::crop_imm(&s.frame, x0, y0, cw, ch).to_image();
    image::imageops::resize(&cropped, s.frame.width(), s.frame.height(), FilterType::Lanczos3)
}
#[allow(clippy::too_many_arguments)]
fn run_export_pipeline(
    save_path: PathBuf,
    items: Vec<ImageItem>,
    title_cfg: TitleScreenConfig,
    img_dur: f32,
    trans_dur: f32,
    trans_type: String,
    music_path: Option<PathBuf>,
    prog_arc: Arc<Mutex<ExportProgress>>,
) -> Result<(), String> {
    let (w, h, fps) = (1920_u32, 1080_u32, 24.0_f32);
    let mut slides: Vec<PreparedSlide> = Vec::new();

    if title_cfg.enabled {
        prog_arc.lock().unwrap().message = "Rendering title screen…".into();
        let base = items.first().map(|it| it.path.as_str());
        let tf = render_title_frame(base, &title_cfg, w, h);
        slides.push(PreparedSlide { frame: tf, duration: title_cfg.duration.max(0.5), kb_dx: 0.0, kb_dy: 0.0 });
    }

    for (idx, it) in items.iter().enumerate() {
        prog_arc.lock().unwrap().message = format!("Loading image {} of {}…", idx + 1, items.len());
        let frame = render_export_frame(it, w, h).ok_or_else(|| format!("Failed to load {}", it.path))?;
        let sign_x = if idx % 2 == 0 { 1.0 } else { -1.0 };
        let sign_y = if idx % 3 == 0 { 1.0 } else { -1.0 };
        slides.push(PreparedSlide {
            frame,
            duration: it.effective_dur(img_dur),
            kb_dx: sign_x * 0.015,
            kb_dy: sign_y * 0.010,
        });
    }

    // Build segment schedule matching calc_video_duration
    let n = slides.len();
    let min_safe = slides.iter().map(|s| trans_dur.min(s.duration / 2.0)).fold(trans_dur, f32::min);
    let mut starts = Vec::with_capacity(n);
    let mut cursor = 0.0;
    for i in 0..n {
        starts.push(cursor);
        if i + 1 < n {
            let eff_safe = trans_dur.min(slides[i].duration / 2.0).min(slides[i + 1].duration / 2.0);
            cursor += match trans_type.as_str() {
                "Crossfade" | "Ken Burns" => slides[i].duration - min_safe,
                "Blur Dissolve" => slides[i].duration + eff_safe,
                _ => slides[i].duration,
            };
        }
    }
    let total_dur = (starts[n - 1] + slides[n - 1].duration).max(0.1);
    let total_frames = (total_dur * fps).round().max(1.0) as usize;
    let fade_out_dur = 3.0_f32.min(total_dur / 2.0);

    // Spawn FFmpeg process (try h264_nvenc first, fallback to libx264)
    let mut child = spawn_ffmpeg(&save_path, w, h, fps as u32, music_path.as_ref(), total_dur, fade_out_dur, "h264_nvenc")
        .or_else(|_| spawn_ffmpeg(&save_path, w, h, fps as u32, music_path.as_ref(), total_dur, fade_out_dur, "libx264"))?;
    let mut stdin = child.stdin.take().ok_or("Failed to open FFmpeg stdin")?;

    let is_kb = trans_type == "Ken Burns";
    for f_idx in 0..total_frames {
        let t = f_idx as f32 / fps;
        if f_idx % 12 == 0 {
            let mut p = prog_arc.lock().unwrap();
            p.progress = f_idx as f32 / total_frames as f32;
            p.message = format!("Encoding video: {}%", (p.progress * 100.0) as u32);
        }

        let mut frame = render_timeline_at_t(&slides, &starts, t, &trans_type, trans_dur, is_kb, w, h);

        // Final 3s fade-to-black
        if t >= total_dur - fade_out_dur && fade_out_dur > 0.0 {
            let gain = ((total_dur - t) / fade_out_dur).clamp(0.0, 1.0);
            frame = scale_brightness(&frame, gain);
        }
        stdin.write_all(frame.as_raw()).map_err(|e| format!("FFmpeg pipe error: {}", e))?;
    }

    drop(stdin);
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("FFmpeg exited with non-zero status".into());
    }
    Ok(())
}

fn render_timeline_at_t(
    slides: &[PreparedSlide], starts: &[f32], t: f32,
    trans_type: &str, trans_dur: f32, is_kb: bool, w: u32, h: u32,
) -> RgbImage {
    // Check overlap or blur-dissolve gap between slide i and i+1
    for i in 0..slides.len().saturating_sub(1) {
        let s0_end = starts[i] + slides[i].duration;
        let s1_start = starts[i + 1];
        if (trans_type == "Crossfade" || trans_type == "Ken Burns") && t >= s1_start && t <= s0_end {
            let alpha = ((t - s1_start) / (s0_end - s1_start).max(0.001)).clamp(0.0, 1.0);
            let fa = sample_slide(&slides[i], t - starts[i], is_kb);
            let fb = sample_slide(&slides[i + 1], t - s1_start, is_kb);
            return blend_rgb(&fa, &fb, alpha);
        }
        if trans_type == "Blur Dissolve" && t > s0_end && t < s1_start {
            let alpha = ((t - s0_end) / (s1_start - s0_end).max(0.001)).clamp(0.0, 1.0);
            let ba = image::imageops::blur(&slides[i].frame, (alpha * 12.0).max(0.1));
            let bb = image::imageops::blur(&slides[i + 1].frame, ((1.0 - alpha) * 12.0).max(0.1));
            return blend_rgb(&ba, &bb, alpha);
        }
    }
    for i in (0..slides.len()).rev() {
        if t >= starts[i] {
            let local_t = (t - starts[i]).clamp(0.0, slides[i].duration);
            let mut img = sample_slide(&slides[i], local_t, is_kb);
            if trans_type == "Fade In/Out" {
                let safe = trans_dur.min(slides[i].duration / 2.0).max(0.001);
                if local_t < safe { img = scale_brightness(&img, local_t / safe); }
                else if local_t > slides[i].duration - safe {
                    img = scale_brightness(&img, (slides[i].duration - local_t) / safe);
                }
            }
            return img;
        }
    }
    RgbImage::new(w, h)
}

fn spawn_ffmpeg(
    out: &PathBuf, w: u32, h: u32, fps: u32,
    music: Option<&PathBuf>, dur: f32, fade_out: f32, codec: &str,
) -> Result<std::process::Child, String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-f", "rawvideo", "-vcodec", "rawvideo", "-pix_fmt", "rgb24",
              "-s", &format!("{}x{}", w, h), "-r", &fps.to_string(), "-i", "-"]);
    if let Some(mp) = music {
        let fade_start = (dur - fade_out).max(0.0);
        let af = format!("atrim=0:{:.3},afade=t=out:st={:.3}:d={:.3}", dur, fade_start, fade_out);
        cmd.args(["-stream_loop", "-1", "-i"]).arg(mp).args(["-af", &af, "-c:a", "aac", "-b:a", "192k"]);
    }
    cmd.args(["-c:v", codec, "-pix_fmt", "yuv420p", "-shortest"]).arg(out)
       .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("Failed to start ffmpeg ({}): {}", codec, e))
}
