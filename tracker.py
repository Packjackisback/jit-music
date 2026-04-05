#!/usr/bin/env python3
import sys
import json
import time
import base64
import cv2
import mediapipe as mp
from mediapipe.tasks import python as mp_python
from mediapipe.tasks.python import vision as mp_vision

MODEL_PATH = "models/hand_landmarker.task"
CAMERA_INDEX = 0
PREVIEW_MAX_WIDTH = 640
PREVIEW_JPEG_QUALITY = 65

HAND_CONNECTIONS = [
    (0, 1), (1, 2), (2, 3), (3, 4),
    (0, 5), (5, 6), (6, 7), (7, 8),
    (5, 9), (9, 10), (10, 11), (11, 12),
    (9, 13), (13, 14), (14, 15), (15, 16),
    (13, 17), (17, 18), (18, 19), (19, 20),
    (0, 17),
]


def draw_landmarks(frame, coords):
    h, w = frame.shape[:2]

    points = []
    for x, y in coords:
        px = max(0, min(w - 1, int(x * w)))
        py = max(0, min(h - 1, int(y * h)))
        points.append((px, py))

    for i, j in HAND_CONNECTIONS:
        cv2.line(frame, points[i], points[j], (80, 220, 80), 2, cv2.LINE_AA)

    for px, py in points:
        cv2.circle(frame, (px, py), 4, (60, 170, 255), -1, cv2.LINE_AA)


def draw_status(frame, text, color):
    cv2.putText(
        frame,
        text,
        (14, 30),
        cv2.FONT_HERSHEY_SIMPLEX,
        0.8,
        color,
        2,
        cv2.LINE_AA,
    )


def make_stream_preview(frame):
    h, w = frame.shape[:2]
    if w > PREVIEW_MAX_WIDTH:
        scale = PREVIEW_MAX_WIDTH / float(w)
        frame = cv2.resize(frame, (PREVIEW_MAX_WIDTH, int(h * scale)), interpolation=cv2.INTER_AREA)

    ok, encoded = cv2.imencode(
        ".jpg",
        frame,
        [int(cv2.IMWRITE_JPEG_QUALITY), PREVIEW_JPEG_QUALITY],
    )
    if not ok:
        return None

    return base64.b64encode(encoded.tobytes()).decode("ascii")

def main():
    base_options = mp_python.BaseOptions(model_asset_path=MODEL_PATH)
    options = mp_vision.HandLandmarkerOptions(
        base_options=base_options,
        running_mode=mp_vision.RunningMode.VIDEO,
        num_hands=1,
        min_hand_detection_confidence=0.5,
        min_hand_presence_confidence=0.5,
        min_tracking_confidence=0.5,
    )

    cap = cv2.VideoCapture(CAMERA_INDEX)
    if not cap.isOpened():
        print(json.dumps({"error": "Could not open camera"}), flush=True)
        sys.exit(1)

    with mp_vision.HandLandmarker.create_from_options(options) as landmarker:
        start_ms = time.monotonic()
        frame_id = 0

        while True:
            ok, frame = cap.read()
            if not ok:
                continue

            frame_id += 1

            rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
            mp_image = mp.Image(image_format=mp.ImageFormat.SRGB, data=rgb)

            timestamp_ms = int((time.monotonic() - start_ms) * 1000)
            result = landmarker.detect_for_video(mp_image, timestamp_ms)

            preview = frame.copy()

            if result.hand_landmarks:
                hand = result.hand_landmarks[0]
                coords = [[lm.x, lm.y] for lm in hand]
                draw_landmarks(preview, coords)
                draw_status(preview, "Hand detected", (80, 220, 80))
            else:
                coords = None
                draw_status(preview, "No hand detected", (180, 180, 180))

            preview_b64 = make_stream_preview(preview)
            line = json.dumps(
                {
                    "frame_id": frame_id,
                    "landmarks": coords,
                    "preview_jpeg_b64": preview_b64,
                }
            )
            print(line, flush=True)

    cap.release()

if __name__ == "__main__":
    main()
