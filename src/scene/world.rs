use crate::resource::immediate_submit::SubmitContext;
use crate::resource::Allocator;
use crate::scene::billboard::Billboard;
use crate::scene::light::LightManager;
use crate::scene::mesh::Mesh;
use crate::scene::model::{Model, ModelId};
use ash::Device;
use egui::ahash::HashMap;
use glam::Vec2;
use glam::{Mat4, Vec4};
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
        if let Some(x) = model.light {
            let child = Model::new(
                vec![],
                Mat4::IDENTITY,
                None,
                Some(Billboard {
                    center: Vec4::ZERO,
                    size: Vec2::from([0.1, 0.1]),
                    uvs: [Vec2::ZERO; 4],
                    material: 0,
                }),
                None,
            );
            let child = self.add_model(child);
            model.children.push(child);
        }

        let id = self.next_free_id();
        model.id = id;
        self.models.insert(id, model);
        self.max_id += 1;
        id
    }

    pub fn get_meshes(&self) -> Vec<&Mesh> {
        self.models.iter().flat_map(|(_, model)| model.meshes.iter()).collect()
    }

    pub fn get_billboards(&self) -> Vec<&Billboard> {
        self.models.iter().filter_map(|(_, model)| model.billboard.as_ref()).collect()
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
                },
                ctx,
            );
        }

        if let Some(billboard) = &mut model.billboard {
            billboard.center = transform.w_axis;
        }

        for child in model.children.clone().as_slice() {
            self.update_transforms(*child, transform, light_manager, ctx);
        }
    }

    pub fn update_billboard(&mut self, billboard: ModelId, center: Vec4, uvs: [Vec2; 4]) {
        let model = self.models.get_mut(&billboard).unwrap();
        model.billboard.as_mut().unwrap().center = center;
        model.billboard.as_mut().unwrap().uvs = uvs;
    }
}
