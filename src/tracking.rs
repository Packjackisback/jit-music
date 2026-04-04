use image::DynamicImage;
use ndarray::Array4;
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::TensorRef,
};
 
use crate::gesture::Landmarks;
 
pub struct HandTracker {
    palm_session: Session,
    landmark_session: Session,
}
 
struct PalmBox {
    cx: f32,
    cy: f32,
    size: f32,
    score: f32,
}
 
impl HandTracker {
    pub fn new() -> Self {
        let palm_session = Session::builder()
            .expect("Failed to create session builder")
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .expect("Failed to set optimization level")
            .commit_from_file("models/palm_detection_full_inf_post_192x192.onnx")
            .expect(
                "Could not load palm model",
            );
 
        let landmark_session = Session::builder()
            .expect("Failed to create session builder")
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .expect("Failed to set optimization level")
            .commit_from_file("models/hand_landmark_sparse_Nx3x224x224.onnx")
            .expect(
                "Could not load landmark model.",
            );
 
        HandTracker { palm_session, landmark_session }
    }
 
    pub fn detect(&mut self, frame: &DynamicImage) -> Option<Landmarks> {
        let palm = self.run_palm_detector(frame)?;
        let (landmarks, hand_score) = self.run_landmark_model(frame, &palm)?;
        if sigmoid(hand_score) < 0.5 {
            return None;
        }
        Some(landmarks)
    }
 
    fn run_palm_detector(&mut self, frame: &DynamicImage) -> Option<PalmBox> {
        let resized = frame.resize_exact(192, 192, image::imageops::FilterType::Triangle);
        let rgb = resized.to_rgb8();
 
        let mut data = Array4::<f32>::zeros([1, 3, 192, 192]);
        for (x, y, pixel) in rgb.enumerate_pixels() {
            data[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
            data[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            data[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
        }
 
        let tensor = TensorRef::from_array_view(data.view()).ok()?;
        let outputs = self
            .palm_session
            .run(ort::inputs!["input" => tensor])
            .ok()?;
 
        let out = outputs["pdscore_boxx_boxy_boxsize_kp0x_kp0y_kp2x_kp2y"]
            .try_extract_array::<f32>()
            .ok()?;
        let n = out.shape()[0];
        if n == 0 {
            return None;
        }
 
        let mut best_score = f32::NEG_INFINITY;
        let mut best: Option<PalmBox> = None;
 
        for i in 0..n {
            let score = sigmoid(out[[i, 0]]);
            if score > best_score {
                best_score = score;
                best = Some(PalmBox {
                    cx:   out[[i, 1]],
                    cy:   out[[i, 2]],
                    size: out[[i, 3]],
                    score,
                });
            }
        }
 
        best.filter(|b| b.score >= 0.5)
    }
 
 
    fn run_landmark_model(
        &mut self,
        frame: &DynamicImage,
        palm: &PalmBox,
    ) -> Option<(Landmarks, f32)> {
        let (fw, fh) = (frame.width() as f32, frame.height() as f32);
 
        let pad = 2.0_f32;
        let half = (palm.size * pad / 2.0).min(0.5);
        let half_px_w = half * fw;
        let half_px_h = half * fh;
        let cx_px = palm.cx * fw;
        let cy_px = palm.cy * fh;
 
        let crop_x = (cx_px - half_px_w).max(0.0) as u32;
        let crop_y = (cy_px - half_px_h).max(0.0) as u32;
        let crop_w = ((half_px_w * 2.0) as u32).min(frame.width().saturating_sub(crop_x));
        let crop_h = ((half_px_h * 2.0) as u32).min(frame.height().saturating_sub(crop_y));
 
        if crop_w == 0 || crop_h == 0 {
            return None;
        }
 
        let crop = frame
            .crop_imm(crop_x, crop_y, crop_w, crop_h)
            .resize_exact(224, 224, image::imageops::FilterType::Triangle);
        let rgb = crop.to_rgb8();
 
        let mut data = Array4::<f32>::zeros([1, 3, 224, 224]);
        for (x, y, pixel) in rgb.enumerate_pixels() {
            data[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
            data[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            data[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
        }
 
        let tensor = TensorRef::from_array_view(data.view()).ok()?;
        let outputs = self
            .landmark_session
            .run(ort::inputs!["input" => tensor])
            .ok()?;
 
        let coords = outputs["xyz_x21"].try_extract_array::<f32>().ok()?;
 
        let scores = outputs["hand_score"].try_extract_array::<f32>().ok()?;
        let hand_score = scores[[0, 0]];
 
        let mut landmarks = [[0f32; 2]; 21];
        for i in 0..21 {
            let lx = (coords[[0, i * 3]]     / 224.0 * crop_w as f32 + crop_x as f32) / fw;
            let ly = (coords[[0, i * 3 + 1]] / 224.0 * crop_h as f32 + crop_y as f32) / fh;
            landmarks[i] = [lx.clamp(0.0, 1.0), ly.clamp(0.0, 1.0)];
        }
 
        Some((landmarks, hand_score))
    }
}
 
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
