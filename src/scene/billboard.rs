use crate::asset::material::MaterialId;
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};

#[derive(Debug, Clone)]
pub struct Billboard {
    pub center: Vec4,
    pub size: Vec2,
    pub uvs: [Vec2; 4],
    pub material: MaterialId,
}

impl Billboard {
    pub fn new(center: Vec4, size: Vec2, uvs: [Vec2; 4], material: MaterialId) -> Self {
        Self {
            center,
            size,
            uvs,
            material,
        }
    }
}
