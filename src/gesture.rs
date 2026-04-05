// 21 landmarks (x, y)
pub type Landmarks = [[f32; 2]; 21];

#[derive(Clone, Default)]
pub struct HandPose {
    pub x: f32,
    pub y: f32,
    pub openness: f32,
    pub rotation: f32,
    pub filter: f32,
    pub fingers_up: usize,
}

fn sq_dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

fn thumb_extended(lm: &Landmarks) -> bool {
    let wrist = lm[0];
    let thumb_ip = lm[3];
    let thumb_tip = lm[4];
    let index_mcp = lm[5];
    let pinky_mcp = lm[17];

    let palm_center = [
        (wrist[0] + index_mcp[0] + pinky_mcp[0]) / 3.0,
        (wrist[1] + index_mcp[1] + pinky_mcp[1]) / 3.0,
    ];

    // Scale margin by palm width so this stays stable across different hand sizes.
    let palm_width = sq_dist(index_mcp, pinky_mcp).sqrt();
    let margin = 0.03 * palm_width;

    let tip_vs_ip_to_index = sq_dist(thumb_tip, index_mcp).sqrt() - sq_dist(thumb_ip, index_mcp).sqrt();
    let tip_vs_ip_to_palm = sq_dist(thumb_tip, palm_center).sqrt() - sq_dist(thumb_ip, palm_center).sqrt();

    tip_vs_ip_to_index > margin && tip_vs_ip_to_palm > margin
}


fn count_extended_fingers(lm: &Landmarks) -> usize {
    // (tip_index, pip_index) pairs for the four fingers
    let fingers = [(8, 6), (12, 10), (16, 14), (20, 18)];
    let mut count = 0;
 
    for (tip, pip) in fingers {
        if lm[tip][1] < lm[pip][1] {
            count += 1;
        }
    }
 
    if thumb_extended(lm) {
        count += 1;
    }
 
    count
}

pub fn analyse(lm: &Landmarks) -> HandPose {
    let fingers_up = count_extended_fingers(lm);
    let x = lm[9][0].clamp(0.0, 1.0);
    let y = lm[9][1].clamp(0.0, 1.0);
    let openness = (fingers_up as f32 / 5.0).clamp(0.0, 1.0);

    let index_mcp = lm[5];
    let pinky_mcp = lm[17];
    let rotation = ((pinky_mcp[1] - index_mcp[1]).atan2(pinky_mcp[0] - index_mcp[0]) + std::f32::consts::PI)
        / (std::f32::consts::TAU);

    HandPose {
        x,
        y,
        openness,
        rotation,
        filter: x,
        fingers_up,
    }
}
