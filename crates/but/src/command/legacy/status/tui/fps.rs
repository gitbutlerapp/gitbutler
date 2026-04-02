use std::time::{Duration, Instant};

#[derive(Debug)]
pub(super) struct FpsCounter {
    timer: Instant,
    next_fps: u32,
    fps: u32,
}

impl FpsCounter {
    pub(super) fn new() -> Self {
        Self {
            timer: Instant::now(),
            fps: 0,
            next_fps: 0,
        }
    }

    pub(super) fn frame_finished(&mut self) {
        self.next_fps += 1;
    }

    pub(super) fn update(&mut self) -> bool {
        if self.timer.elapsed() > Duration::from_secs(1) {
            self.fps = std::mem::take(&mut self.next_fps);
            self.timer = Instant::now();
            true
        } else {
            false
        }
    }

    pub(super) fn fps(&self) -> u32 {
        self.fps
    }
}
