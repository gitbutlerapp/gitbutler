use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct FpsCounter {
    timer: Instant,
    next_fps: u32,
    fps: u32,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            timer: Instant::now(),
            fps: 0,
            next_fps: 0,
        }
    }

    pub fn frame_finished(&mut self) {
        self.next_fps += 1;
    }

    pub fn update(&mut self) -> bool {
        if self.timer.elapsed() > Duration::from_secs(1) {
            self.fps = std::mem::take(&mut self.next_fps);
            self.timer = Instant::now();
            true
        } else {
            false
        }
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }
}
