use crate::resource::Allocator;
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
}

impl Model {
    pub fn destroy(&mut self, device: &Device, allocator: &mut Allocator) {
        for mut mesh in self.meshes.drain(..) {
            mesh.destroy(device, allocator);
        }
    }
}
