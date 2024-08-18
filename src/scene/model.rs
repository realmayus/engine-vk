use crate::resource::Allocator;
use crate::scene::billboard::Billboard;
use crate::scene::light::LightId;
use crate::scene::mesh::Mesh;
use ash::Device;
use glam::Mat4;

pub type ModelId = usize;

#[derive(Default)]
pub struct Model {
    pub id: ModelId,
    pub meshes: Vec<Mesh>,
    pub children: Vec<ModelId>,
    pub label: Option<String>,
    pub transform: Mat4,
    pub light: Option<LightId>,
    pub billboard: Option<Billboard>,
}

impl Model {
    pub fn new(
        meshes: Vec<Mesh>,
        transform: Mat4,
        light: Option<LightId>,
        mut billboard: Option<Billboard>,
        label: Option<String>,
    ) -> Self {
        Self {
            id: 0,
            meshes,
            children: Vec::new(),
            label,
            transform,
            light,
            billboard,
        }
    }
    pub fn destroy(&mut self, device: &Device, allocator: &mut Allocator) {
        for mut mesh in self.meshes.drain(..) {
            mesh.destroy(device, allocator);
        }
    }
}
