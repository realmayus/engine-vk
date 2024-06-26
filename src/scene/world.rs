use crate::resource::immediate_submit::SubmitContext;
use crate::resource::Allocator;
use crate::scene::light::LightManager;
use crate::scene::mesh::Mesh;
use crate::scene::model::{Model, ModelId};
use ash::Device;
use egui::ahash::HashMap;
use glam::{Mat4, Vec4, Vec4Swizzles};
use hashbrown::HashSet;

#[derive(Default)]
pub struct World {
    pub models: HashMap<ModelId, Model>,
    max_id: ModelId,
}

impl World {
    pub fn next_free_id(&self) -> ModelId {
        self.max_id + 1
    }

    pub fn add_model(&mut self, mut model: Model) -> ModelId {
        let id = self.next_free_id();
        model.id = id;
        self.models.insert(id, model);
        self.max_id += 1;
        id
    }

    pub fn get_meshes(&self) -> Vec<&Mesh> {
        self.models.iter().flat_map(|(_, model)| model.meshes.iter()).collect()
    }

    pub fn get_toplevel_model_ids(&self) -> Vec<ModelId> {
        let all_children = self
            .models
            .iter()
            .flat_map(|(_, model)| model.children.iter())
            .collect::<HashSet<_>>();
        self.models
            .keys()
            .copied()
            .filter(|model| !all_children.contains(&model))
            .collect::<Vec<_>>()
    }

    pub fn destroy(&mut self, device: &Device, allocator: &mut Allocator) {
        for (_, mut model) in self.models.drain() {
            model.destroy(device, allocator);
        }
    }

    /*
    Called e.g. when loading a new scene
     */
    pub fn clear(&mut self, device: &Device, allocator: &mut Allocator) {
        for (_, mut model) in self.models.drain() {
            model.destroy(device, allocator);
        }
    }

    pub fn update_transforms(&mut self, model: ModelId, parent: Mat4, light_manager: &mut LightManager, ctx: &mut SubmitContext) {
        let model = self.models.get_mut(&model).unwrap();
        let transform = parent * model.transform;
        for mesh in model.meshes.as_mut_slice() {
            mesh.transform = transform;
        }
        if let Some(light) = model.light {
            light_manager.update_light(
                light,
                |light| {
                    light.position = transform.w_axis.to_array();
                    // light.direction = (transform * -Vec4::Y).normalize().to_array();
                    light.direction = (-Vec4::Y).normalize().to_array();
                },
                ctx,
            );
        }
        for child in model.children.clone().as_slice() {
            self.update_transforms(*child, transform, light_manager, ctx);
        }
    }
}
