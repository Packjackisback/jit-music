use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use crate::gesture::Landmarks;

#[derive(Clone)]
pub struct TrackedHand {
    pub landmarks: Landmarks,
    pub handedness: Option<String>,
}

type SharedHands = Arc<Mutex<Option<Vec<TrackedHand>>>>;
type SharedPreview = Arc<Mutex<Option<(u64, Vec<u8>)>>>;

pub struct HandTracker {
    latest: SharedHands,
    latest_preview: SharedPreview,
    _child: Child,
}

impl HandTracker {
    pub fn new() -> Self {
        let mut child = Command::new("python3")
            .arg("tracker.py")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect(
                "Failed to spawn tracker.py.",
            );

        let stdout = child
            .stdout
            .take()
            .expect("Failed to capture tracker.py stdout");

        let latest: SharedHands = Arc::new(Mutex::new(None));
        let latest_preview: SharedPreview = Arc::new(Mutex::new(None));
        let latest_clone = latest.clone();
        let latest_preview_clone = latest_preview.clone();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = match line {
                    Ok(l)  => l,
                    Err(e) => { eprintln!("[tracker] read error: {e}"); break; }
                };

                if let Some((hands, preview)) = parse_line(&line) {
                    if let Some(parsed_hands) = hands {
                        if let Ok(mut guard) = latest_clone.lock() {
                            *guard = Some(parsed_hands);
                        }
                    }

                    if let Some((frame_id, jpeg)) = preview {
                        if let Ok(mut guard) = latest_preview_clone.lock() {
                            *guard = Some((frame_id, jpeg));
                        }
                    }
                }
            }
            eprintln!("[tracker] Python process ended");
        });

        HandTracker { latest, latest_preview, _child: child }
    }

    pub fn latest_hands(&self) -> Option<Vec<TrackedHand>> {
        self.latest.lock().ok()?.clone()
    }

    pub fn latest_preview_jpeg(&self) -> Option<(u64, Vec<u8>)> {
        self.latest_preview.lock().ok()?.clone()
    }
}

fn parse_line(line: &str) -> Option<(Option<Vec<TrackedHand>>, Option<(u64, Vec<u8>)>)> {
    if line.is_empty() { return None; }

    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let hands = parse_hands(&v);
    let preview = parse_preview(&v);

    Some((hands, preview))
}

fn parse_landmarks_value(value: &serde_json::Value) -> Option<Landmarks> {
    let arr = value.as_array()?;
    if arr.len() != 21 { return None; }

    let mut landmarks = [[0f32; 2]; 21];
    for (i, point) in arr.iter().enumerate() {
        let coords = point.as_array()?;
        landmarks[i] = [
            coords.get(0)?.as_f64()? as f32,
            coords.get(1)?.as_f64()? as f32,
        ];
    }

    Some(landmarks)
}

fn parse_hands(v: &serde_json::Value) -> Option<Vec<TrackedHand>> {
    if let Some(hands) = v["hands"].as_array() {
        let mut parsed = Vec::new();

        for hand in hands {
            let landmarks = parse_landmarks_value(hand.get("landmarks")?)?;
            let handedness = hand
                .get("handedness")
                .and_then(|value| value.as_str())
                .map(str::to_owned);
            parsed.push(TrackedHand { landmarks, handedness });
        }

        if !parsed.is_empty() {
            return Some(parsed);
        }
    }

    let landmarks = parse_landmarks_value(v.get("landmarks")?)?;
    Some(vec![TrackedHand { landmarks, handedness: None }])
}

fn parse_preview(v: &serde_json::Value) -> Option<(u64, Vec<u8>)> {
    let frame_id = v["frame_id"].as_u64()?;
    let b64 = v["preview_jpeg_b64"].as_str()?;
    let jpeg = BASE64_STANDARD.decode(b64).ok()?;
    Some((frame_id, jpeg))
}
