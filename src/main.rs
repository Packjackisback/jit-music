mod audio;
mod camera;
mod gesture;
mod tracking;

use audio::AudioEngine;
use camera::CameraCapture;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use gesture::GestureState;
use tracking::HandTracker;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
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
    camera:         CameraCapture,
    tracker:        HandTracker,
    gesture:        GestureState,
    camera_texture: Option<TextureHandle>,
}

impl App {
    fn new() -> Self {
        App {
            audio:          AudioEngine::new(),
            camera:         CameraCapture::new(),
            tracker:        HandTracker::new(),
            gesture:        GestureState::default(),
            camera_texture: None,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if let Some(frame) = self.camera.latest_frame() {
            if let Some(landmarks) = self.tracker.detect(&frame) {
                self.gesture = gesture::analyse(&landmarks);
            } else {
                self.gesture = GestureState::default();
            }

            if self.gesture.is_playing {
                self.audio.set_note(self.gesture.frequency_hz, self.gesture.volume);
            } else {
                self.audio.silence();
            }

            if let Some(rgb8) = frame.as_rgb8() {
                let w = rgb8.width() as usize;
                let h = rgb8.height() as usize;
                let pixels: Vec<egui::Color32> = rgb8
                    .pixels()
                    .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
                    .collect();

                let color_image = ColorImage {
                    size:        [w, h],
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
            }
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Camera").strong());
                    if let Some(tex) = &self.camera_texture {
                        let w = 480.0_f32;
                        let h = w / tex.aspect_ratio();
                        ui.image((tex.id(), egui::vec2(w, h)));
                    } else {
                        ui.add_sized(
                            [480.0, 360.0],
                            egui::Label::new(
                                egui::RichText::new("Waiting for camera…")
                                    .color(egui::Color32::GRAY),
                            ),
                        );
                    }
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("jit-music").heading().strong());
                    ui.add_space(8.0);

                    let g = &self.gesture;
                    stat(ui, "Status",    if g.is_playing { "playing" } else { "silent" });
                    stat(ui, "Note",      &if g.is_playing { g.note_name.clone() } else { "—".into() });
                    stat(ui, "Frequency", &format!("{:.1} Hz", g.frequency_hz));
                    stat(ui, "Volume",    &format!("{:.0}%", g.volume * 100.0));
                    stat(ui, "Fingers",   &g.fingers_up.to_string());

                    ui.add_space(14.0);
                    ui.label("Volume");
                    let bar_w = ui.available_width().min(200.0);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_w, 14.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(40));
                    let fill = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(rect.width() * g.volume, rect.height()),
                    );
                    ui.painter().rect_filled(
                        fill,
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
