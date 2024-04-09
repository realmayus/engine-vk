use crate::resource::Allocator;
use crate::scene::mesh::Mesh;
use crate::scene::model::{Model, ModelId};
use ash::Device;
use glam::Mat4;
use hashbrown::HashSet;

#[derive(Default)]
pub struct World {
    pub models: Vec<Model>,
}

impl World {
    pub fn next_free_id(&self) -> ModelId {
        self.models.len()
    }

    pub fn add_model(&mut self, mut model: Model) -> ModelId {
        let id = self.next_free_id();
        model.id = id;
        self.models.push(model);
        id
    }

    pub fn get_meshes(&self) -> Vec<&Mesh> {
        self.models.iter().flat_map(|model| model.meshes.iter()).collect()
    }

    pub fn get_toplevel_model_ids(&self) -> Vec<ModelId> {
        let all_children = self.models.iter().flat_map(|model| model.children.iter()).collect::<HashSet<_>>();
        self.models
            .iter()
            .map(|m| m.id)
            .filter(|model| !all_children.contains(&model))
            .collect::<Vec<_>>()
    }

    pub fn destroy(&mut self, device: &Device, allocator: &mut Allocator) {
        for mut model in self.models.drain(..) {
            model.destroy(device, allocator);
        }
    }

    /*
    Called e.g. when loading a new scene
     */
    pub fn clear(&mut self, device: &Device, allocator: &mut Allocator) {
        for mut model in self.models.drain(..) {
            model.destroy(device, allocator);
        }
    }

    pub fn update_transforms(&mut self, model: ModelId, parent: Mat4) {
        let model = self.models.get_mut(model).unwrap();
        let transform = parent * model.transform;
        for mesh in model.meshes.as_mut_slice() {
            mesh.parent_transform = transform;
        }
        for child in model.children.clone().as_slice() {
            self.update_transforms(*child, transform);
        }
    }
}
