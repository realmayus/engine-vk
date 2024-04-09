use crate::asset::material::MaterialManager;
use crate::asset::material::{Material, MaterialId, PbrMaterial, RawMaterial};
use crate::asset::texture::{Texture, TextureId, TextureManager};
use crate::resource::immediate_submit::SubmitContext;
use crate::resource::Allocator;
use crate::scene::mesh::Mesh;
use crate::scene::model::{Model, ModelId};
use crate::scene::world::World;
use ash::{vk, Device};
use glam::{Mat4, Vec2, Vec3};
use hashbrown::HashMap;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

pub struct GltfReader {
    world: Rc<RefCell<World>>,
    texture_manager: Rc<RefCell<TextureManager>>,
    material_manager: Rc<RefCell<MaterialManager>>,
    cleanups: Vec<Box<dyn FnOnce(&Device, &mut Allocator)>>,
    material_mappings: HashMap<usize, MaterialId>,
}

struct ImageData {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl GltfReader {
    pub fn new(
        world: Rc<RefCell<World>>,
        texture_manager: Rc<RefCell<TextureManager>>,
        material_manager: Rc<RefCell<MaterialManager>>,
    ) -> Self {
        Self {
            world,
            texture_manager,
            material_manager,
            cleanups: Vec::new(),
            material_mappings: HashMap::new(),
        }
    }

    pub fn load(&mut self, path: &Path, ctx: SubmitContext) {
        let (gltf, buffers, images) = gltf::import(path).unwrap();
        let images = images.iter().map(|image| image.to_rgba8()).collect::<Vec<_>>();
        let images = images
            .iter()
            .map(|image| ImageData {
                width: image.width(),
                height: image.height(),
                data: image.to_vec(),
            })
            .collect::<Vec<_>>();
        let device = ctx.device.clone();
        ctx.immediate_submit(Box::new(|ctx| {
            let mut mapping = HashMap::<usize, ModelId>::new();
            for node in gltf.nodes() {
                let model = self.load_model(&node, &buffers, &images, ctx, Mat4::IDENTITY);
                mapping.insert(node.index(), model);
            }
            for node in gltf.nodes() {
                let model = mapping.get(&node.index()).unwrap();
                for child in node.children() {
                    self.world.borrow_mut().models[*model]
                        .children
                        .push(*mapping.get(&child.index()).unwrap());
                }
                self.world.borrow_mut().update_transforms(*model, Mat4::IDENTITY);
            }
        }));
        self.texture_manager.borrow_mut().update_set(&device);
    }

    fn load_model(
        &mut self,
        node: &gltf::Node,
        buffers: &Vec<gltf::buffer::Data>,
        images: &[ImageData],
        ctx: &mut SubmitContext,
        parent_transform: Mat4,
    ) -> ModelId {
        let mut model = Model {
            label: node.name().map(|x| x.to_string()),
            ..Default::default()
        };
        let node_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
        for mesh in node.mesh().iter() {
            for primitive in mesh.primitives() {
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                let mut normals = Vec::new();
                let mut uvs = Vec::new();
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(iter) = reader.read_positions() {
                    for position in iter {
                        vertices.push(Vec3::from(position));
                    }
                }
                if let Some(iter) = reader.read_indices() {
                    for index in iter.into_u32() {
                        indices.push(index);
                    }
                }
                if let Some(iter) = reader.read_normals() {
                    for normal in iter {
                        normals.push(Vec3::from(normal));
                    }
                }
                if let Some(iter) = reader.read_tex_coords(0) {
                    for uv in iter.into_f32() {
                        uvs.push(Vec2::from(uv));
                    }
                }
                let gltf_material = primitive.material();
                let material_id = if let Some(index) = gltf_material.index() {
                    self.material_mappings
                        .get(&index)
                        .copied()
                        .unwrap_or_else(|| self.load_material(gltf_material, images, ctx))
                } else {
                    0 as MaterialId
                };
                let mut mesh = Mesh {
                    mem: None,
                    vertices,
                    indices,
                    normals,
                    uvs,
                    material: material_id,
                    parent_transform,
                    transform: parent_transform * node_transform,
                };
                ctx.nest(Box::new(|ctx| {
                    mesh.upload(ctx);
                }));
                model.meshes.push(mesh);
            }
        }
        self.world.borrow_mut().add_model(model)
    }

    fn load_material(&mut self, material: gltf::Material, images: &[ImageData], ctx: &mut SubmitContext) -> MaterialId {
        if material.index().is_none() {
            return 0;
        }

        let pbr = material.pbr_metallic_roughness();
        let albedo = pbr.base_color_factor();
        let texture = pbr.base_color_texture().map(|info| {
            let image = images.get(info.texture().source().index()).unwrap();

            let texture = ctx.nest(Box::new(|ctx| {
                Texture::new(
                    TextureManager::DEFAULT_SAMPLER_NEAREST,
                    vk::Format::R8G8B8A8_SRGB,
                    ctx,
                    Some(info.texture().name().map(|x| x.to_string()).unwrap_or(format!(
                        "Albedo, Material: {} ({})",
                        material.name().unwrap_or_default(),
                        self.material_manager.borrow().next_free_id()
                    ))),
                    &image.data,
                    vk::Extent3D {
                        width: image.width,
                        height: image.height,
                        depth: 1,
                    },
                )
            }));
            self.texture_manager.borrow_mut().add_texture(texture, &ctx.device, false)
        });
        let engine_material = ctx.nest(Box::new(|ctx| {
            Material::new(
                Some(material.name().unwrap_or_default().to_string()),
                RawMaterial::Pbr(PbrMaterial {
                    texture: texture.unwrap_or(0 as TextureId),
                    albedo,
                    metallic: pbr.metallic_factor(),
                    roughness: pbr.roughness_factor(),
                    padding: 0.0,
                }),
                ctx,
            )
        }));

        let engine_material = self.material_manager.borrow_mut().add_material(engine_material);
        self.material_mappings.insert(material.index().unwrap(), engine_material);
        engine_material
    }
}
