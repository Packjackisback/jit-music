The way I see it I have 4 main parts:

Get the camera - nokhwa
Get the landmarks - ort with 2 ONNX models
Get the gestures - my code
Convert the gestures to audio - My code
Play the audio - cpal



## camera
Ideally I want a platform-agnostic camera solution, and nokwah seemed like the best solution from a quick google search.

## landmarks

This is harder. At first I was interested in getting the mediapipe-rs wasm solution to work (https://github.com/WasmEdge/mediapipe-rs) but I realised quickly it wouldn't wpork for my use case. Instead, I have to use 2 models, both from https://github.com/PINTO0309/hand-gesture-recognition-using-onnx:

- https://github.com/PINTO0309/hand-gesture-recognition-using-onnx/blob/main/model/palm_detection/palm_detection_full_inf_post_192x192.onnx
- https://github.com/PINTO0309/hand-gesture-recognition-using-onnx/blob/main/model/hand_landmark/hand_landmark_sparse_Nx3x224x224.onnx

I used netron to look at the input/outputs:

### Palm Detector
Input:  
        "input"  float32[1, 3, 192, 192]
Output: 
        "pdscore_boxx_boxy_boxsize_kp0x_kp0y_kp2x_kp2y"  float32[N, 8]

[0] pd_score - detection confidence (apply sigmoid to get probability)
[1] box_x    - palm center x, normalized [0,1]
[2] box_y    - palm center y, normalized [0,1]
[3] box_size - palm size as fraction of image
[4] kp0_x    - keypoint 0 x (wrist)
[5] kp0_y    - keypoint 0 y
[6] kp2_x    - keypoint 2 x (middle finger base)
[7] kp2_y    - keypoint 2 y

### Hand Landmarks

Input:  
        "input"      float32[N, 3, 224, 224]
Output: 
        "xyz_x21"    float32[N, 63]   - 21 landmarks × (x, y, z)
        "hand_score" float32[N, 1]    - landmark confidence
        "lefthand_0_or_righthand_1" float32[N, 1]

## Conversion

We are given 21 hand landmarks, and we need to convert them into musical params

0 = wrist
1-4 = thumb (MCP, IP, TIP)
5-8 = index finger (MCP, PIP, DIP, tip)
9-12 = middle finger
13-16 = ring finger
17-20 = pinky

As per usual, all the coords are normalized, 00 is top/left, 1.0 is bottom/right.

Counting the number of extended fingers is a little tricky. The traditional approach I know is comparing the y of the knuckle vs the fingertip, but this didn't work for the thumb. I compared the x on the thumb instead.

## Music

I am absolutely tone-deaf. My limited musical knowledge told me that the pentatonic sounds nice, but I heavily relied on the internet for almost anything music related.

I was inspired by a similar project I saw on instagram, which was a lot more complicated, involving tracking. That's a little too complicated for a speed build, so I'm going to avoid too much complexity, keeping this a simple poc.

Because of that, I'm going to just do volume = palm horizontal, pitch = vertical, fist = silence. I eventually will do stuff with number of fingers, rotation of hand, hand opening angle, etc.

## Postmortem

This is suprisingly easy to do. Ort makes this really painless, and I'm interested in doing more things with it. Currently, getting the number of fingers extended is insanely buggy, and it seems like the landmark model just isn't accurate enough. I'm not sure if it is a consequence of what i'm passing it, but I think it just isn't the most accurate model. The obvious solution is to switch to mediapipe, either through the cpp/python bindings, or this: https://github.com/julesyoungberg/mediapipe-rs.


# Round 2

mediapipe-rs is just not a good solution in this case. Instead, I'm going to have to use the python binding :hs:

For simplicity's sake, I switched the camera logic to the python part of the project
