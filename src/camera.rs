use image::DynamicImage;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub struct CameraCapture {
    latest: Arc<Mutex<Option<DynamicImage>>>,
    running: Arc<AtomicBool>,
    _thread: std::thread::JoinHandle<()>,
}

impl CameraCapture {
    pub fn new() -> Self {
        let latest: Arc<Mutex<Option<DynamicImage>>> = Arc::new(Mutex::new(None));
        let running = Arc::new(AtomicBool::new(true));
 
        let latest_clone = latest.clone();
        let running_clone = running.clone();
 
        let thread = std::thread::spawn(move || {
            let format = RequestedFormat::new::<RgbFormat>(
                RequestedFormatType::AbsoluteHighestResolution,
            );
            let mut camera = Camera::new(CameraIndex::Index(0), format)
                .expect(
                    "Could not open webcam.",
                );
            camera.open_stream().expect("Failed to open camera stream");
 
            while running_clone.load(Ordering::Relaxed) {
                match camera.frame() {
                    Ok(buffer) => {
                        if let Ok(rgb) = buffer.decode_image::<RgbFormat>() {
                            let dynamic = DynamicImage::ImageRgb8(
                                image::ImageBuffer::from_raw(
                                    rgb.width(),
                                    rgb.height(),
                                    rgb.into_raw(),
                                )
                                .expect("Buffer size mismatch — this is a bug"),
                            );
                            if let Ok(mut guard) = latest_clone.lock() {
                                *guard = Some(dynamic);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[camera] frame error: {e}");
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
        });
 
        CameraCapture {
            latest,
            running,
            _thread: thread,
        }
    }
 
    pub fn latest_frame(&self) -> Option<DynamicImage> {
        self.latest.lock().ok()?.clone()
    }
}
 
impl Drop for CameraCapture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
