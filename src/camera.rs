use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use std::mem;

use winit::event::MouseScrollDelta;

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub fov: f32,
    pub dirty: bool,
    pub extent: (f32, f32),
    drag: bool,
    delta: (f32, f32),
}

impl Camera {
    const UP: Vec3 = Vec3::new(0.0, -1.0, 0.0);
    const NEAR: f32 = 0.1;
    const FAR: f32 = 10000.0;
    pub fn new(width: f32, height: f32) -> Self {
        let position = Vec3::new(2.0, 3.0, 5.0);
        let target = Vec3::ZERO;
        let fov = 60.0f32;
        Self {
            position,
            extent: (width, height),
            target,
            fov,
            dirty: true,
            drag: false,
            delta: (0.0, 0.0),
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.extent = (width, height);
        self.dirty = true;
    }

    pub fn reset(&mut self) {
        let new = Self::new(self.extent.0, self.extent.1);
        let _ = mem::replace(self, new);
        self.dirty = true;
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_lh(self.position, self.target, Self::UP)
    }

    pub fn proj(&self) -> Mat4 {
        Mat4::perspective_lh(self.fov.to_radians(), self.extent.0 / self.extent.1, Self::NEAR, Self::FAR)
    }

    pub fn on_mouse_move(&mut self, delta: (f32, f32)) {
        if self.drag {
            let x_angle = (delta.0) * (2.0 * std::f32::consts::PI / self.extent.0);
            let mut y_angle = (delta.1 * 2.0) * (std::f32::consts::PI / self.extent.1);

            let view_dir = (self.target - self.position).normalize();
            let cos_angle = view_dir.dot(Self::UP);
            if cos_angle * y_angle.signum() > 0.99 {
                y_angle = 0.0;
            }

            let rot_mat_x = Mat4::from_axis_angle(-Self::UP, -x_angle);
            self.position = ((rot_mat_x * Vec4::from(((self.position - self.target), 1.0))) + Vec4::from((self.target, 1.0))).xyz();
            let rot_mat_y = Mat4::from_axis_angle(view_dir.cross(Self::UP), y_angle);
            self.position = ((rot_mat_y * Vec4::from(((self.position - self.target), 1.0))) + Vec4::from((self.target, 1.0))).xyz();

            self.dirty = true;
        }
    }

    pub fn on_mouse_scroll(&mut self, delta: MouseScrollDelta) {
        match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                let y = -y;
                let view_dir = (self.target - self.position).normalize();
                let new_pos = self.position + view_dir * y;
                if new_pos.distance(self.target) > 0.1 {
                    self.position = new_pos;
                    self.dirty = true;
                }
            }
            MouseScrollDelta::PixelDelta(_) => {}
        }
    }

    pub fn on_mouse_drag(&mut self, drag: bool) {
        self.drag = drag;
        self.delta = (0.0, 0.0);
    }
}
