use glam::{Mat4, Vec3};
use std::mem;

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub fov: f32,
    pub dirty: bool,
    extent: (f32, f32),
}

impl Camera {
    const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    const NEAR: f32 = 10000.0;
    const FAR: f32 = 0.1;
    pub fn new(width: f32, height: f32) -> Self {
        let position = Vec3::new(2.0, 3.0, 5.0);
        let target = Vec3::ZERO;
        let fov = 60.0f32;
        let mut proj = Mat4::perspective_rh(fov.to_radians(), width / height, Self::NEAR, Self::FAR);
        proj.y_axis.y *= -1.0;
        Self {
            position,
            extent: (width, height),
            target,
            fov,
            dirty: true,
        }
    }

    pub fn reset(&mut self) {
        let new = Self::new(self.extent.0, self.extent.1);
        let _ = mem::replace(self, new);
        self.dirty = true;
    }

    pub fn view(&mut self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, Self::UP)
    }

    pub fn proj(&mut self) -> Mat4 {
        let mut proj = Mat4::perspective_rh(self.fov.to_radians(), self.extent.0 / self.extent.1, Self::NEAR, Self::FAR);
        proj.y_axis.y *= -1.0;
        proj
    }
}
