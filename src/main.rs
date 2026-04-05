mod audio;
mod gesture;
mod tracking;

use audio::AudioEngine;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use gesture::HandPose;
use tracking::HandTracker;
use std::time::{Duration, Instant};

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
    camera_texture: Option<TextureHandle>,
    last_preview_frame_id: Option<u64>,
    previous_pose: Option<HandPose>,
    previous_frame_at: Option<Instant>,
    steady_since: Option<Instant>,
    previous_avg_y: Option<f32>,
    loop_index: usize,
    hold_active: bool,
    build_amount: f32,
    last_swipe_at: Option<Instant>,
    last_tap_at: Option<Instant>,
    last_drop_at: Option<Instant>,
    latest_event: String,
    active_hands: usize,
}

impl App {
    fn new() -> Self {
        App {
            audio:          AudioEngine::new(),
            tracker:        HandTracker::new(),
            camera_texture: None,
            last_preview_frame_id: None,
            previous_pose: None,
            previous_frame_at: None,
            steady_since: None,
            previous_avg_y: None,
            loop_index: 0,
            hold_active: false,
            build_amount: 0.0,
            last_swipe_at: None,
            last_tap_at: None,
            last_drop_at: None,
            latest_event: String::from("ready"),
            active_hands: 0,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        let dt = self
            .previous_frame_at
            .map(|previous| now.duration_since(previous).as_secs_f32())
            .unwrap_or(1.0 / 60.0)
            .clamp(1.0 / 240.0, 0.25);
        self.previous_frame_at = Some(now);

        let hands = self.tracker.latest_hands().unwrap_or_default();
        self.active_hands = hands.len();
        let primary_handedness = hands
            .first()
            .and_then(|hand| hand.handedness.as_deref())
            .unwrap_or("unknown");

        let primary_pose = hands.first().map(|hand| gesture::analyse(&hand.landmarks));
        let average_y = if hands.is_empty() {
            None
        } else {
            Some(hands.iter().map(|hand| hand.landmarks[9][1]).sum::<f32>() / hands.len() as f32)
        };

        let mut master_volume = 0.0;
        let mut filter_cutoff = 0.0;
        let mut intensity_target = 0.0;
        let mut delay_wet = 0.0;
        let mut dial = 0.0;

        if let Some(pose) = primary_pose.clone() {
            master_volume = (0.25 + pose.openness * 0.75).clamp(0.0, 1.0);
            filter_cutoff = pose.filter;
            delay_wet = pose.y;
            dial = pose.rotation;

            if let Some(previous_pose) = &self.previous_pose {
                let dx = pose.x - previous_pose.x;
                let dy = pose.y - previous_pose.y;
                let speed = (dx * dx + dy * dy).sqrt() / dt;
                let is_swipe = dx.abs() > 0.14 && dx.abs() > dy.abs() * 1.4 && speed > 1.0;
                let is_tap = dy.abs() > 0.12 && dy.abs() > dx.abs() * 1.2 && speed > 0.9;
                let is_stable = speed < 0.08;

                if is_stable {
                    if self.steady_since.is_none() {
                        self.steady_since = Some(now);
                    }
                } else {
                    self.steady_since = None;
                }

                self.hold_active = pose.openness > 0.65
                    && self
                        .steady_since
                        .map(|since| now.duration_since(since) >= Duration::from_millis(450))
                        .unwrap_or(false);

                if is_swipe {
                    let cooldown_passed = self
                        .last_swipe_at
                        .map(|time| now.duration_since(time) >= Duration::from_millis(350))
                        .unwrap_or(true);
                    if cooldown_passed {
                        if dx > 0.0 {
                            self.loop_index = (self.loop_index + 1) % audio::LOOP_NAMES.len();
                        } else {
                            self.loop_index = (self.loop_index + audio::LOOP_NAMES.len() - 1) % audio::LOOP_NAMES.len();
                        }
                        self.last_swipe_at = Some(now);
                        self.latest_event = format!("loop: {}", audio::LOOP_NAMES[self.loop_index]);
                    }
                }

                if is_tap {
                    let cooldown_passed = self
                        .last_tap_at
                        .map(|time| now.duration_since(time) >= Duration::from_millis(220))
                        .unwrap_or(true);
                    if cooldown_passed {
                        self.audio.trigger_sample();
                        self.last_tap_at = Some(now);
                        self.latest_event = String::from("tap: sample");
                    }
                }

                if let Some(previous_avg_y) = self.previous_avg_y {
                    let current_avg_y = average_y.unwrap_or(previous_avg_y);
                    let drop_movement = current_avg_y - previous_avg_y;
                    let cooldown_passed = self
                        .last_drop_at
                        .map(|time| now.duration_since(time) >= Duration::from_millis(700))
                        .unwrap_or(true);
                    if cooldown_passed && drop_movement > 0.14 && current_avg_y > 0.66 {
                        self.audio.trigger_drop();
                        self.last_drop_at = Some(now);
                        self.latest_event = String::from("drop: impact");
                        self.build_amount = 0.0;
                        self.hold_active = false;
                    }
                }
            } else {
                self.steady_since = None;
            }

            self.previous_pose = Some(pose);
        } else {
            self.previous_pose = None;
            self.steady_since = None;
            self.hold_active = false;
            self.latest_event = String::from("waiting for hands");
        }

        if let Some(avg_y) = average_y {
            intensity_target = (1.0 - avg_y).clamp(0.0, 1.0);
            self.previous_avg_y = Some(avg_y);
        } else {
            self.previous_avg_y = None;
        }

        if self.hold_active {
            self.build_amount = self.build_amount.max(intensity_target);
        } else {
            self.build_amount += (intensity_target - self.build_amount) * 0.06;
        }

        self.audio.set_scene(
            master_volume,
            filter_cutoff,
            self.build_amount,
            delay_wet,
            dial,
            self.loop_index,
            self.hold_active,
        );

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
                egui::vec2(rect.width().min(340.0), 355.0),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(overlay_rect), |ui| {
                egui::Frame::default()
                    .fill(egui::Color32::from_black_alpha(170))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(90)))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("jit-music").heading().strong());
                        ui.add_space(6.0);
                        ui.label("x -> filter");
                        ui.label("hand openness -> volume");
                        ui.label("y -> delay wet");
                        ui.label("dial rotation -> delay time");
                        ui.label("swipe -> loop switch");
                        ui.label("tap -> sample");
                        ui.label("hold -> sustain build");
                        ui.label("raise / drop -> energy");
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);

                        stat(ui, "Hands", &self.active_hands.to_string());
                        stat(ui, "Primary", primary_handedness);
                        stat(ui, "Loop", audio::LOOP_NAMES[self.loop_index]);
                        stat(ui, "Openness", &format!("{:.0}%", primary_pose.as_ref().map(|pose| pose.openness * 100.0).unwrap_or(0.0)));
                        stat(ui, "Dial", &format!("{:.0}%", primary_pose.as_ref().map(|pose| pose.rotation * 100.0).unwrap_or(0.0)));
                        stat(ui, "Hold", if self.hold_active { "sustaining" } else { "moving" });
                        stat(ui, "Event", &self.latest_event);
                        if let Some(pose) = self.previous_pose.as_ref() {
                            stat(ui, "Fingers", &pose.fingers_up.to_string());
                        }
                        stat(ui, "Build", &format!("{:.0}%", self.build_amount * 100.0));

                        ui.add_space(10.0);
                        ui.label("Energy");
                        let bar_w = ui.available_width();
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, 14.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(bar_rect, 4.0, egui::Color32::from_gray(40));
                        let fill_rect = egui::Rect::from_min_size(
                            bar_rect.min,
                            egui::vec2(bar_rect.width() * master_volume, bar_rect.height()),
                        );
                        ui.painter().rect_filled(
                            fill_rect,
                            4.0,
                            egui::Color32::from_rgb(80, 200, 110),
                        );

                        ui.add_space(8.0);
                        ui.label("Filter");
                        let (filter_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, 14.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(filter_rect, 4.0, egui::Color32::from_gray(40));
                        let filter_fill = egui::Rect::from_min_size(
                            filter_rect.min,
                            egui::vec2(filter_rect.width() * filter_cutoff, filter_rect.height()),
                        );
                        ui.painter().rect_filled(
                            filter_fill,
                            4.0,
                            egui::Color32::from_rgb(90, 150, 255),
                        );

                        ui.add_space(8.0);
                        ui.label("Delay wet");
                        let (intensity_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, 14.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(intensity_rect, 4.0, egui::Color32::from_gray(40));
                        let intensity_fill = egui::Rect::from_min_size(
                            intensity_rect.min,
                            egui::vec2(intensity_rect.width() * delay_wet, intensity_rect.height()),
                        );
                        ui.painter().rect_filled(
                            intensity_fill,
                            4.0,
                            egui::Color32::from_rgb(255, 170, 70),
                        );

                        ui.add_space(8.0);
                        ui.label("Dial");
                        let (dial_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, 14.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(dial_rect, 4.0, egui::Color32::from_gray(40));
                        let dial_fill = egui::Rect::from_min_size(
                            dial_rect.min,
                            egui::vec2(dial_rect.width() * dial, dial_rect.height()),
                        );
                        ui.painter().rect_filled(
                            dial_fill,
                            4.0,
                            egui::Color32::from_rgb(180, 120, 255),
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
