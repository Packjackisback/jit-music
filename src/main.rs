mod audio;
mod gesture;
mod tracking;

use audio::AudioEngine;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use gesture::GestureState;
use tracking::HandTracker;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("jit-music")
            .with_inner_size([820.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
        "jit-music",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()) as Box<dyn eframe::App>)),
    )
}

struct App {
    audio:          AudioEngine,
    tracker:        HandTracker,
    gesture:        GestureState,
    camera_texture: Option<TextureHandle>,
    last_preview_frame_id: Option<u64>,
}

impl App {
    fn new() -> Self {
        App {
            audio:          AudioEngine::new(),
            tracker:        HandTracker::new(),
            gesture:        GestureState::default(),
            camera_texture: None,
            last_preview_frame_id: None,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if let Some(landmarks) = self.tracker.latest_landmarks() {
            self.gesture = gesture::analyse(&landmarks);
        } else {
            self.gesture = GestureState::default();
        }

        if self.gesture.is_playing {
            self.audio.set_note(self.gesture.frequency_hz, self.gesture.volume);
        } else {
            self.audio.silence();
        }

        if let Some((frame_id, jpeg)) = self.tracker.latest_preview_jpeg() {
            let is_new_frame = self
                .last_preview_frame_id
                .map(|id| id != frame_id)
                .unwrap_or(true);

            if is_new_frame {
                if let Ok(dynamic) = image::load_from_memory(&jpeg) {
                    let rgb8 = dynamic.to_rgb8();
                    let w = rgb8.width() as usize;
                    let h = rgb8.height() as usize;
                    let pixels: Vec<egui::Color32> = rgb8
                        .pixels()
                        .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
                        .collect();
                    let color_image = ColorImage {
                        size: [w, h],
                        source_size: egui::vec2(w as f32, h as f32),
                        pixels,
                    };

                    match &mut self.camera_texture {
                        Some(tex) => tex.set(color_image, TextureOptions::LINEAR),
                        None => {
                            self.camera_texture = Some(ctx.load_texture(
                                "camera",
                                color_image,
                                TextureOptions::LINEAR,
                            ));
                        }
                    }

                    self.last_preview_frame_id = Some(frame_id);
                }
            }
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let rect = ui.max_rect();

            if let Some(tex) = &self.camera_texture {
                ui.put(
                    rect,
                    egui::Image::new((tex.id(), rect.size())),
                );
            } else {
                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(20));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Waiting for tracker preview…",
                    egui::TextStyle::Heading.resolve(ui.style()),
                    egui::Color32::GRAY,
                );
            }

            let overlay_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(16.0, 16.0),
                egui::vec2(rect.width().min(320.0), 260.0),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(overlay_rect), |ui| {
                egui::Frame::default()
                    .fill(egui::Color32::from_black_alpha(170))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(90)))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Hand Music").heading().strong());
                        ui.add_space(6.0);
                        ui.label("hand height -> pitch");
                        ui.label("hand x position -> volume");
                        ui.label("fist -> silence");
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);

                        let g = &self.gesture;
                        stat(ui, "Status", if g.is_playing { "playing" } else { "silent" });
                        stat(ui, "Note", &if g.is_playing { g.note_name.clone() } else { "—".into() });
                        stat(ui, "Frequency", &format!("{:.1} Hz", g.frequency_hz));
                        stat(ui, "Volume", &format!("{:.0}%", g.volume * 100.0));
                        stat(ui, "Fingers", &g.fingers_up.to_string());

                        ui.add_space(10.0);
                        ui.label("Volume");
                        let bar_w = ui.available_width();
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, 14.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(bar_rect, 4.0, egui::Color32::from_gray(40));
                        let fill_rect = egui::Rect::from_min_size(
                            bar_rect.min,
                            egui::vec2(bar_rect.width() * g.volume, bar_rect.height()),
                        );
                        ui.painter().rect_filled(
                            fill_rect,
                            4.0,
                            egui::Color32::from_rgb(80, 200, 110),
                        );
                    });
            });
        });

        ctx.request_repaint();
    }
}

fn stat(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{label}:")).color(egui::Color32::GRAY));
        ui.label(egui::RichText::new(value).strong());
    });
}
